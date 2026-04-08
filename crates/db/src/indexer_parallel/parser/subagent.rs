// crates/db/src/indexer_parallel/parser/subagent.rs
// Subagent workload merging: merge parse results and recompute productivity metrics.

use claude_view_core::count_ai_lines;

use super::file_io::parse_file_bytes;
use crate::indexer_parallel::helpers::*;
use crate::indexer_parallel::types::*;

pub(crate) fn merge_subagent_parse_result(parent: &mut ParseResult, subagent: ParseResult) {
    // Preserve existing token + turn merge behavior.
    parent.deep.total_input_tokens += subagent.deep.total_input_tokens;
    parent.deep.total_output_tokens += subagent.deep.total_output_tokens;
    parent.deep.cache_read_tokens += subagent.deep.cache_read_tokens;
    parent.deep.cache_creation_tokens += subagent.deep.cache_creation_tokens;

    // Merge raw invocation-backed productivity signals so parent metrics reflect
    // parent + subagent workload under one attributed session.
    parent.deep.tool_counts.edit += subagent.deep.tool_counts.edit;
    parent.deep.tool_counts.read += subagent.deep.tool_counts.read;
    parent.deep.tool_counts.bash += subagent.deep.tool_counts.bash;
    parent.deep.tool_counts.write += subagent.deep.tool_counts.write;
    parent.deep.tool_call_count += subagent.deep.tool_call_count;
    parent.deep.files_read.extend(subagent.deep.files_read);
    parent.deep.files_edited.extend(subagent.deep.files_edited);
    parent
        .deep
        .files_touched
        .extend(subagent.deep.files_touched);
    parent.deep.skills_used.extend(subagent.deep.skills_used);
    parent
        .deep
        .hook_progress_events
        .extend(subagent.deep.hook_progress_events);

    for model in subagent.models_seen {
        if !parent.models_seen.contains(&model) {
            parent.models_seen.push(model);
        }
    }
    parent.turns.extend(subagent.turns);
    parent.raw_invocations.extend(subagent.raw_invocations);
}

fn recompute_merged_productivity_metrics(result: &mut ParseResult) {
    // files_read: keep deduplicated vector + count contract
    let mut files_read_unique = std::mem::take(&mut result.deep.files_read);
    files_read_unique.sort();
    files_read_unique.dedup();
    result.deep.files_read_count = files_read_unique.len() as u32;
    result.deep.files_read = files_read_unique;

    // files_edited: preserve all occurrences for re-edit derivation
    let mut files_edited_unique = result.deep.files_edited.clone();
    files_edited_unique.sort();
    files_edited_unique.dedup();
    result.deep.files_edited_count = files_edited_unique.len() as u32;
    result.deep.reedited_files_count = count_reedited_files(&result.deep.files_edited);

    // ai_lines_* are derived from raw invocations, so recompute from merged set.
    let ai_line_count = count_ai_lines(
        result
            .raw_invocations
            .iter()
            .map(|inv| (inv.name.as_str(), &inv.input))
            .filter_map(|(name, input)| input.as_ref().map(|i| (name, i))),
    );
    result.deep.ai_lines_added = ai_line_count.lines_added;
    result.deep.ai_lines_removed = ai_line_count.lines_removed;

    result.deep.skills_used.sort();
    result.deep.skills_used.dedup();
    result.deep.files_touched.sort();
    result.deep.files_touched.dedup();
}

pub(crate) fn merge_subagent_workload(
    parent_jsonl_path: &std::path::Path,
    result: &mut ParseResult,
) {
    let subagent_dir = parent_jsonl_path.with_extension("").join("subagents");
    if !subagent_dir.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(&subagent_dir) else {
        return;
    };

    let mut subagent_jsonls: Vec<std::path::PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        .collect();
    subagent_jsonls.sort();

    if subagent_jsonls.is_empty() {
        return;
    }

    for sub_path in subagent_jsonls {
        let sub_result = parse_file_bytes(&sub_path);
        merge_subagent_parse_result(result, sub_result);
    }

    recompute_merged_productivity_metrics(result);
}
