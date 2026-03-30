// crates/server/src/local_llm/download.rs

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

// ---- Public types ----

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f32>,
    pub file_name: Option<String>,
    pub files_done: u32,
    pub files_total: u32,
    pub speed_bytes_per_sec: Option<u64>,
    pub eta_secs: Option<u64>,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---- Planning (pure, testable) ----

#[derive(Debug, PartialEq)]
pub enum FileAction {
    /// File exists with correct size — skip.
    Skip,
    /// .partial file exists — resume from this byte offset.
    Resume { from_byte: u64 },
    /// Fresh download needed.
    Download,
}

/// Append ".partial" to a path.
pub fn partial_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".partial");
    PathBuf::from(s)
}

/// Decide whether to skip, resume, or download a single file.
pub fn plan_file_download(local_path: &Path, expected_size: Option<u64>) -> FileAction {
    // Complete file exists?
    if local_path.exists() {
        if let Ok(meta) = std::fs::metadata(local_path) {
            match expected_size {
                Some(expected) if meta.len() == expected => return FileAction::Skip,
                Some(_) => { /* size mismatch — re-download */ }
                None if meta.len() > 0 => return FileAction::Skip,
                None => { /* empty file — re-download */ }
            }
        }
    }

    // Partial file exists?
    let partial = partial_path(local_path);
    if partial.exists() {
        if let Ok(meta) = std::fs::metadata(&partial) {
            if meta.len() > 0 {
                return FileAction::Resume {
                    from_byte: meta.len(),
                };
            }
        }
    }

    FileAction::Download
}

// ---- HuggingFace API types ----

#[derive(Deserialize)]
struct HfModelInfo {
    siblings: Vec<HfSibling>,
}

#[derive(Deserialize)]
struct HfSibling {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    lfs: Option<HfLfs>,
}

#[derive(Deserialize)]
struct HfLfs {
    size: u64,
}

impl HfSibling {
    fn file_size(&self) -> Option<u64> {
        self.lfs.as_ref().map(|l| l.size).or(self.size)
    }
}

struct PlannedFile {
    name: String,
    url: String,
    local_path: PathBuf,
    expected_size: Option<u64>,
    action: FileAction,
}

// ---- Progress tracking (private) ----

struct ProgressTracker {
    tx: mpsc::Sender<DownloadProgress>,
    bytes_downloaded: u64,
    total_bytes: u64,
    files_done: u32,
    files_total: u32,
    current_file: String,
    started_at: Instant,
    /// Exponential moving average of speed (bytes/sec), smoothed to avoid jitter.
    ema_speed: f64,
}

impl ProgressTracker {
    fn speed_and_eta(&self) -> (Option<u64>, Option<u64>) {
        let speed = if self.ema_speed > 0.0 {
            Some(self.ema_speed as u64)
        } else {
            None
        };
        let eta = if self.ema_speed > 100.0 && self.total_bytes > self.bytes_downloaded {
            let remaining = self.total_bytes - self.bytes_downloaded;
            Some((remaining as f64 / self.ema_speed) as u64)
        } else {
            None
        };
        (speed, eta)
    }

    fn update_speed(&mut self, chunk_bytes: u64, elapsed: Duration) {
        let secs = elapsed.as_secs_f64();
        if secs > 0.0 {
            let instant_speed = chunk_bytes as f64 / secs;
            // EMA with α=0.3 — responsive but smooth
            if self.ema_speed == 0.0 {
                self.ema_speed = instant_speed;
            } else {
                self.ema_speed = 0.3 * instant_speed + 0.7 * self.ema_speed;
            }
        }
    }

    async fn report(&self) {
        let percent = if self.total_bytes > 0 {
            Some(self.bytes_downloaded as f32 / self.total_bytes as f32 * 100.0)
        } else {
            None
        };
        let (speed, eta) = self.speed_and_eta();
        let _ = self
            .tx
            .send(DownloadProgress {
                bytes_downloaded: self.bytes_downloaded,
                total_bytes: Some(self.total_bytes),
                percent,
                file_name: Some(self.current_file.clone()),
                files_done: self.files_done,
                files_total: self.files_total,
                speed_bytes_per_sec: speed,
                eta_secs: eta,
                done: false,
                error: None,
            })
            .await;
    }

    async fn report_done(&self) {
        let _ = self
            .tx
            .send(DownloadProgress {
                bytes_downloaded: self.bytes_downloaded,
                total_bytes: Some(self.total_bytes),
                percent: Some(100.0),
                file_name: None,
                files_done: self.files_total,
                files_total: self.files_total,
                speed_bytes_per_sec: None,
                eta_secs: None,
                done: true,
                error: None,
            })
            .await;
    }

    async fn report_error(&self, error: String) {
        let _ = self
            .tx
            .send(DownloadProgress {
                bytes_downloaded: self.bytes_downloaded,
                total_bytes: Some(self.total_bytes),
                percent: None,
                file_name: None,
                files_done: self.files_done,
                files_total: self.files_total,
                speed_bytes_per_sec: None,
                eta_secs: None,
                done: true,
                error: Some(error),
            })
            .await;
    }
}

// ---- Download execution ----

/// Download an entire HuggingFace model repo to `model_dir`.
/// Skips already-complete files, resumes partial downloads.
/// Cancellable via the provided token. Returns the number of files.
pub async fn download_repo(
    hf_repo: &str,
    model_dir: &Path,
    tx: mpsc::Sender<DownloadProgress>,
    cancel: CancellationToken,
) -> Result<u32, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    // 1. Fetch file list from HF API
    let api_url = format!("https://huggingface.co/api/models/{hf_repo}");
    let info: HfModelInfo = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("HF API request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("HF API parse failed: {e}"))?;

    // 2. Plan downloads
    tokio::fs::create_dir_all(model_dir)
        .await
        .map_err(|e| format!("create model dir: {e}"))?;

    let files: Vec<PlannedFile> = info
        .siblings
        .iter()
        .map(|s| {
            let local_path = model_dir.join(&s.rfilename);
            let expected_size = s.file_size();
            let action = plan_file_download(&local_path, expected_size);
            PlannedFile {
                name: s.rfilename.clone(),
                url: format!(
                    "https://huggingface.co/{}/resolve/main/{}",
                    hf_repo, s.rfilename
                ),
                local_path,
                expected_size,
                action,
            }
        })
        .collect();

    let total_bytes: u64 = files.iter().filter_map(|f| f.expected_size).sum();
    let files_total = files.len() as u32;

    // Pre-count already-complete bytes
    let bytes_downloaded: u64 = files
        .iter()
        .filter(|f| f.action == FileAction::Skip)
        .filter_map(|f| f.expected_size)
        .sum();
    let files_done: u32 = files
        .iter()
        .filter(|f| f.action == FileAction::Skip)
        .count() as u32;

    let mut progress = ProgressTracker {
        tx,
        bytes_downloaded,
        total_bytes,
        files_done,
        files_total,
        current_file: String::new(),
        started_at: Instant::now(),
        ema_speed: 0.0,
    };

    // 3. Download each file
    for file in &files {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }

        if file.action == FileAction::Skip {
            continue;
        }

        let initial_from_byte = match &file.action {
            FileAction::Resume { from_byte } => *from_byte,
            _ => 0,
        };

        progress.current_file = file.name.clone();
        progress.report().await;

        // Retry loop (3 attempts with exponential backoff)
        let mut attempt = 0u32;
        loop {
            if cancel.is_cancelled() {
                return Err("cancelled".into());
            }

            // On retry, re-stat .partial to get actual offset (previous attempt may
            // have written bytes before failing).
            let from_byte = if attempt > 0 {
                partial_path(&file.local_path)
                    .metadata()
                    .map(|m| m.len())
                    .unwrap_or(0)
            } else {
                initial_from_byte
            };

            match download_file(
                &client,
                &file.url,
                &file.local_path,
                from_byte,
                &mut progress,
                &cancel,
            )
            .await
            {
                Ok(()) => break,
                Err(e) if e == "cancelled" => return Err(e),
                Err(e) if attempt < 3 && is_retryable(&e) => {
                    attempt += 1;
                    let delay = Duration::from_secs(1 << attempt);
                    warn!(%e, attempt, file = %file.name, "retrying download");
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(format!("failed to download {}: {e}", file.name)),
            }
        }

        progress.files_done += 1;
    }

    // 4. Final done signal
    progress.report_done().await;
    Ok(files_total)
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    from_byte: u64,
    progress: &mut ProgressTracker,
    cancel: &CancellationToken,
) -> Result<(), String> {
    let partial = partial_path(dest);

    // Ensure parent directory exists
    if let Some(parent) = partial.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create dir: {e}"))?;
    }

    let mut req = client.get(url);
    if from_byte > 0 {
        req = req.header("Range", format!("bytes={from_byte}-"));
    }

    let mut resp = req.send().await.map_err(|e| format!("request: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return Err(format!("HTTP {status}"));
    }

    let mut file = if from_byte > 0 {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&partial)
            .await
            .map_err(|e| format!("open partial: {e}"))?
    } else {
        tokio::fs::File::create(&partial)
            .await
            .map_err(|e| format!("create partial: {e}"))?
    };

    let mut last_report = Instant::now();
    let mut chunk_bytes_since_report = 0u64;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                drop(file);
                return Err("cancelled".into());
            }
            chunk_result = resp.chunk() => {
                match chunk_result.map_err(|e| format!("read: {e}"))? {
                    Some(chunk) => {
                        file.write_all(&chunk)
                            .await
                            .map_err(|e| format!("write: {e}"))?;
                        let len = chunk.len() as u64;
                        progress.bytes_downloaded += len;
                        chunk_bytes_since_report += len;

                        // Throttle progress reports to ~4/sec
                        let elapsed = last_report.elapsed();
                        if elapsed > Duration::from_millis(250) {
                            progress.update_speed(chunk_bytes_since_report, elapsed);
                            progress.report().await;
                            chunk_bytes_since_report = 0;
                            last_report = Instant::now();
                        }
                    }
                    None => break,
                }
            }
        }
    }
    drop(file);

    // Atomic rename: .partial → final
    tokio::fs::rename(&partial, dest)
        .await
        .map_err(|e| format!("rename: {e}"))?;

    Ok(())
}

fn is_retryable(error: &str) -> bool {
    error.contains("429")
        || error.contains("timed out")
        || error.contains("connection")
        || error.contains("reset")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn partial_path_appends_suffix() {
        let p = partial_path(Path::new("/tmp/model.safetensors"));
        assert_eq!(p, PathBuf::from("/tmp/model.safetensors.partial"));
    }

    #[test]
    fn plan_skip_when_file_exists_with_correct_size() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("weights.bin");
        std::fs::write(&file, vec![0u8; 1000]).unwrap();

        assert_eq!(plan_file_download(&file, Some(1000)), FileAction::Skip,);
    }

    #[test]
    fn plan_download_when_size_mismatch() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("weights.bin");
        std::fs::write(&file, vec![0u8; 500]).unwrap();

        assert_eq!(plan_file_download(&file, Some(1000)), FileAction::Download,);
    }

    #[test]
    fn plan_skip_when_no_expected_size_but_file_nonempty() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("config.json");
        std::fs::write(&file, b"{}").unwrap();

        assert_eq!(plan_file_download(&file, None), FileAction::Skip);
    }

    #[test]
    fn plan_resume_when_partial_exists() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("weights.bin");
        let partial = dir.path().join("weights.bin.partial");
        std::fs::write(&partial, vec![0u8; 500]).unwrap();

        assert_eq!(
            plan_file_download(&file, Some(1000)),
            FileAction::Resume { from_byte: 500 },
        );
    }

    #[test]
    fn plan_download_when_nothing_exists() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("weights.bin");

        assert_eq!(plan_file_download(&file, Some(1000)), FileAction::Download,);
    }

    #[test]
    fn hf_sibling_file_size_prefers_lfs() {
        let s = HfSibling {
            rfilename: "model.safetensors".into(),
            size: Some(100),
            lfs: Some(HfLfs { size: 900_000_000 }),
        };
        assert_eq!(s.file_size(), Some(900_000_000));
    }

    #[test]
    fn hf_sibling_file_size_falls_back_to_size() {
        let s = HfSibling {
            rfilename: "config.json".into(),
            size: Some(2048),
            lfs: None,
        };
        assert_eq!(s.file_size(), Some(2048));
    }

    #[test]
    fn hf_sibling_file_size_none_when_missing() {
        let s = HfSibling {
            rfilename: "readme.md".into(),
            size: None,
            lfs: None,
        };
        assert_eq!(s.file_size(), None);
    }
}
