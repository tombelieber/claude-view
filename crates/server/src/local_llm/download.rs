// crates/server/src/local_llm/download.rs

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
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
    pub done: bool,
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
}

impl ProgressTracker {
    async fn report(&self) {
        let percent = if self.total_bytes > 0 {
            Some(self.bytes_downloaded as f32 / self.total_bytes as f32 * 100.0)
        } else {
            None
        };
        let _ = self
            .tx
            .send(DownloadProgress {
                bytes_downloaded: self.bytes_downloaded,
                total_bytes: Some(self.total_bytes),
                percent,
                file_name: Some(self.current_file.clone()),
                files_done: self.files_done,
                files_total: self.files_total,
                done: false,
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
                files_done: self.files_done,
                files_total: self.files_total,
                done: true,
            })
            .await;
    }
}

// ---- Download execution ----

/// Download an entire HuggingFace model repo to `model_dir`.
/// Skips already-complete files, resumes partial downloads.
/// Returns the number of files in the completed download.
pub async fn download_repo(
    hf_repo: &str,
    model_dir: &Path,
    tx: mpsc::Sender<DownloadProgress>,
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
    };

    // 3. Download each file
    for file in &files {
        if file.action == FileAction::Skip {
            continue;
        }

        let from_byte = match &file.action {
            FileAction::Resume { from_byte } => *from_byte,
            _ => 0,
        };

        progress.current_file = file.name.clone();
        progress.report().await;

        // Retry loop (3 attempts with exponential backoff)
        let mut attempt = 0u32;
        loop {
            match download_file(
                &client,
                &file.url,
                &file.local_path,
                from_byte,
                &mut progress,
            )
            .await
            {
                Ok(()) => break,
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
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("read: {e}"))? {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
        progress.bytes_downloaded += chunk.len() as u64;

        // Throttle progress reports to ~4/sec
        if last_report.elapsed() > Duration::from_millis(250) {
            progress.report().await;
            last_report = Instant::now();
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
