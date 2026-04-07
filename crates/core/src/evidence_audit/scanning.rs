//! File scanning and parallel directory traversal for JSONL evidence files.

use super::extraction::extract_line_signals;
use super::types::AggregatedSignals;
use crate::live_parser::{parse_single_line, TailFinders};
use crate::pipeline_checks::{
    run_per_line_checks, run_per_session_checks, LineOffsets, PipelineSignals,
};
use std::path::{Path, PathBuf};

/// Trim leading/trailing ASCII whitespace from a byte slice (MSRV-safe).
fn trim_ascii_bytes(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|p| p + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

/// Load and parse a baseline JSON file.
pub fn load_baseline(path: &Path) -> Result<super::types::Baseline, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read baseline {}: {}", path.display(), e))?;
    serde_json::from_slice(&data).map_err(|e| format!("Failed to parse baseline JSON: {}", e))
}

/// Scan a single JSONL file and return aggregated signals.
pub(crate) fn scan_file(path: &Path) -> AggregatedSignals {
    let mut agg = AggregatedSignals {
        files_scanned: 1,
        ..Default::default()
    };

    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            agg.errors += 1;
            return agg;
        }
    };

    let newline = b'\n';
    let mut start = 0;
    for pos in memchr::memchr_iter(newline, &data) {
        let line = trim_ascii_bytes(&data[start..pos]);
        start = pos + 1;
        if line.is_empty() {
            continue;
        }
        agg.lines_scanned += 1;
        let signals = extract_line_signals(line);
        if signals.top_level_type.is_none() {
            agg.errors += 1;
        }
        agg.ingest(signals);
    }

    // Handle last line (no trailing newline)
    let last = trim_ascii_bytes(&data[start..]);
    if !last.is_empty() {
        agg.lines_scanned += 1;
        let signals = extract_line_signals(last);
        if signals.top_level_type.is_none() {
            agg.errors += 1;
        }
        agg.ingest(signals);
    }

    agg
}

/// Discover JSONL files in a Claude Code data directory (2-level walk).
///
/// Structure: `data_dir/<project>/<uuid>.jsonl`
///
/// Also descends into `<project>/<uuid>/subagents/*.jsonl` for sub-agent logs.
pub fn discover_jsonl_files(data_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let projects = match std::fs::read_dir(data_dir) {
        Ok(rd) => rd,
        Err(_) => return files,
    };

    for project_entry in projects.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(&project_path) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();

            if entry_path.extension().is_some_and(|ext| ext == "jsonl") {
                // Direct session JSONL: <project>/<uuid>.jsonl
                files.push(entry_path);
            } else if entry_path.is_dir() {
                // Session subdirectory: <project>/<uuid>/subagents/*.jsonl
                let subagents = entry_path.join("subagents");
                if subagents.is_dir() {
                    if let Ok(sub_entries) = std::fs::read_dir(&subagents) {
                        for sub_entry in sub_entries.flatten() {
                            let sub_path = sub_entry.path();
                            if sub_path.extension().is_some_and(|ext| ext == "jsonl") {
                                files.push(sub_path);
                            }
                        }
                    }
                }
            }
        }
    }

    files
}

/// Scan all JSONL files in a data directory in parallel using scoped threads.
pub fn scan_directory_parallel(data_dir: &Path) -> AggregatedSignals {
    let files = discover_jsonl_files(data_dir);
    if files.is_empty() {
        return AggregatedSignals::default();
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunk_size = files.len().div_ceil(num_threads);
    let chunks: Vec<&[PathBuf]> = files.chunks(chunk_size).collect();

    let mut final_agg = AggregatedSignals::default();

    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    let mut local = AggregatedSignals::default();
                    for path in chunk {
                        local.merge(scan_file(path));
                    }
                    local
                })
            })
            .collect();

        for handle in handles {
            if let Ok(partial) = handle.join() {
                final_agg.merge(partial);
            }
        }
    });

    final_agg
}

// ─── Phase 1+2 Combined Scanner ─────────────────────────────────

/// Scan a single JSONL file for Phase 1 (type coverage) + Phase 2 (pipeline invariants).
pub fn scan_file_with_pipeline(path: &Path) -> (AggregatedSignals, PipelineSignals) {
    let finders = TailFinders::new();
    let mut agg = AggregatedSignals {
        files_scanned: 1,
        ..Default::default()
    };
    let mut pipeline = PipelineSignals::default();

    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            agg.errors += 1;
            return (agg, pipeline);
        }
    };

    let file_name = path.display().to_string();
    let mut session_lines: Vec<(LineOffsets, crate::live_parser::LiveLine)> = Vec::new();

    let newline = b'\n';
    let mut start = 0;

    // Process a single line: Phase 1 signals + Phase 2 per-line checks + collect for per-session
    let process_line =
        |line_bytes: &[u8],
         line_start: usize,
         line_end: usize,
         agg: &mut AggregatedSignals,
         pipeline: &mut PipelineSignals,
         session_lines: &mut Vec<(LineOffsets, crate::live_parser::LiveLine)>| {
            agg.lines_scanned += 1;

            // Phase 1: type coverage (lightweight serde visitors)
            let signals = extract_line_signals(line_bytes);
            if signals.top_level_type.is_none() {
                agg.errors += 1;
            }
            agg.ingest(signals);

            // Phase 2: parse with actual parser
            let parsed = parse_single_line(line_bytes, &finders);
            if let Ok(raw_value) = serde_json::from_slice::<serde_json::Value>(line_bytes) {
                run_per_line_checks(&raw_value, &parsed, &file_name, agg.lines_scanned, pipeline);
            }

            // Phase 3: field inventory
            let field_inv = crate::field_inventory::extract_field_inventory(line_bytes);
            agg.field_inventory.merge(&field_inv);

            // Store offset + parsed line for per-session checks (no line copy)
            session_lines.push((
                LineOffsets {
                    start: line_start,
                    end: line_end,
                },
                parsed,
            ));
        };

    for pos in memchr::memchr_iter(newline, &data) {
        let line = trim_ascii_bytes(&data[start..pos]);
        if !line.is_empty() {
            let trimmed_start = line.as_ptr() as usize - data.as_ptr() as usize;
            let trimmed_end = trimmed_start + line.len();
            process_line(
                line,
                trimmed_start,
                trimmed_end,
                &mut agg,
                &mut pipeline,
                &mut session_lines,
            );
        }
        start = pos + 1;
    }

    // Handle last line (no trailing newline)
    let last = trim_ascii_bytes(&data[start..]);
    if !last.is_empty() {
        let trimmed_start = last.as_ptr() as usize - data.as_ptr() as usize;
        let trimmed_end = trimmed_start + last.len();
        process_line(
            last,
            trimmed_start,
            trimmed_end,
            &mut agg,
            &mut pipeline,
            &mut session_lines,
        );
    }

    // Per-session checks — pass pre-parsed lines + raw bytes via offsets
    let line_pairs: Vec<(&[u8], crate::live_parser::LiveLine)> = session_lines
        .into_iter()
        .map(|(offsets, parsed)| (offsets.slice(&data), parsed))
        .collect();
    run_per_session_checks(&line_pairs, &file_name, &mut pipeline);

    (agg, pipeline)
}

/// Parallel scanner running both Phase 1 and Phase 2.
pub fn scan_directory_parallel_with_pipeline(
    data_dir: &Path,
) -> (AggregatedSignals, PipelineSignals) {
    let files = discover_jsonl_files(data_dir);
    if files.is_empty() {
        return (AggregatedSignals::default(), PipelineSignals::default());
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunk_size = files.len().div_ceil(num_threads);
    let chunks: Vec<&[PathBuf]> = files.chunks(chunk_size).collect();

    let mut final_agg = AggregatedSignals::default();
    let mut final_pipeline = PipelineSignals::default();

    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    let mut local_agg = AggregatedSignals::default();
                    let mut local_pipeline = PipelineSignals::default();
                    for path in chunk {
                        let (agg, pipeline) = scan_file_with_pipeline(path);
                        local_agg.merge(agg);
                        local_pipeline.merge(pipeline);
                    }
                    (local_agg, local_pipeline)
                })
            })
            .collect();

        for handle in handles {
            if let Ok((partial_agg, partial_pipeline)) = handle.join() {
                final_agg.merge(partial_agg);
                final_pipeline.merge(partial_pipeline);
            }
        }
    });

    (final_agg, final_pipeline)
}
