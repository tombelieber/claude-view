//! Feature vector extraction from a sliding window of StepSignals.
//!
//! Converts a window of 10 steps into a 96-element f32 vector that matches
//! the Python training pipeline (extract_signals.py + train_and_benchmark.py).

use super::StepSignals;

pub const N_FEATURES: usize = 96;
const DECAY: f32 = 0.85;

/// Convert a window of steps into a flat feature vector for the XGBoost model.
///
/// Feature layout (96 total):
///   [0..16]   Aggregated counts (sum across window)
///   [16..19]  Total edit, total read, edit/read ratio
///   [19..51]  Boolean OR flags (any step has signal)
///   [51..83]  Count of steps with each flag
///   [83..91]  Decay-weighted sums (recent steps weighted higher)
///   [91..96]  Structural features
pub fn flatten_window(steps: &[StepSignals]) -> [f32; N_FEATURES] {
    let mut f = [0.0f32; N_FEATURES];
    let n = steps.len();
    if n == 0 {
        return f;
    }

    // --- Aggregated counts [0..16] ---
    for s in steps {
        f[0] += s.edit_count as f32;
        f[1] += s.write_count as f32;
        f[2] += s.read_count as f32;
        f[3] += s.glob_count as f32;
        f[4] += s.grep_count as f32;
        f[5] += s.bash_count as f32;
        f[6] += s.agent_count as f32;
        f[7] += s.skill_count as f32;
        f[8] += s.todo_count as f32;
        f[9] += s.config_files_edited as f32;
        f[10] += s.test_files_edited as f32;
        f[11] += s.doc_files_edited as f32;
        f[12] += s.plan_files_edited as f32;
        f[13] += s.script_files_edited as f32;
        f[14] += s.ci_files_edited as f32;
        f[15] += s.migration_files_edited as f32;
    }

    // --- Total edit, read, ratio [16..19] ---
    let total_edit: f32 = steps
        .iter()
        .map(|s| (s.edit_count + s.write_count) as f32)
        .sum();
    let total_read: f32 = steps
        .iter()
        .map(|s| (s.read_count + s.glob_count + s.grep_count) as f32)
        .sum();
    f[16] = total_edit;
    f[17] = total_read;
    f[18] = total_edit / total_read.max(1.0);

    // --- Boolean OR flags [19..51] (32 flags) ---
    let bool_fns: [fn(&StepSignals) -> bool; 32] = [
        |s| s.has_plan_skill,
        |s| s.has_review_skill,
        |s| s.has_test_skill,
        |s| s.has_ship_skill,
        |s| s.has_debug_skill,
        |s| s.has_config_skill,
        |s| s.has_impl_skill,
        |s| s.has_explore_skill,
        |s| s.has_plan_agent,
        |s| s.has_review_agent,
        |s| s.has_explore_agent,
        |s| s.has_test_cmd,
        |s| s.has_build_cmd,
        |s| s.has_git_push,
        |s| s.has_publish_cmd,
        |s| s.has_deploy_cmd,
        |s| s.has_install_cmd,
        |s| s.has_review_combo,
        |s| s.has_plan_execute_combo,
        |s| s.has_tdd_combo,
        |s| s.has_git_commit,
        |s| s.has_git_diff,
        |s| s.has_docker_cmd,
        |s| s.has_lint_cmd,
        |s| s.prompt_plan_kw,
        |s| s.prompt_impl_kw,
        |s| s.prompt_fix_kw,
        |s| s.prompt_review_kw,
        |s| s.prompt_test_kw,
        |s| s.prompt_release_kw,
        |s| s.prompt_config_kw,
        |s| s.prompt_explore_kw,
    ];

    for (i, flag_fn) in bool_fns.iter().enumerate() {
        // OR flag: any step has it
        f[19 + i] = if steps.iter().any(flag_fn) { 1.0 } else { 0.0 };
        // Count: how many steps have it
        f[51 + i] = steps.iter().filter(|s| flag_fn(s)).count() as f32;
    }

    // --- Decay-weighted sums [83..91] (8 signals) ---
    for (i, s) in steps.iter().enumerate() {
        let age = (n - 1 - i) as f32;
        let w = DECAY.powf(age);

        f[83] += w * (s.edit_count + s.write_count) as f32;
        f[84] += w * (s.read_count + s.glob_count + s.grep_count) as f32;
        f[85] += w * s.bash_count as f32;
        f[86] += w * s.agent_count as f32;
        f[87] += w * s.skill_count as f32;
        f[88] += w * if s.has_test_cmd { 1.0 } else { 0.0 };
        f[89] += w * if s.has_git_push { 1.0 } else { 0.0 };
        f[90] += w * if s.is_user_prompt { 1.0 } else { 0.0 };
    }

    // --- Structural features [91..96] ---
    // User prompt count
    f[91] = steps.iter().filter(|s| s.is_user_prompt).count() as f32;

    // Position of first edit (normalized)
    f[92] = steps
        .iter()
        .position(|s| s.edit_count > 0)
        .map(|p| p as f32 / n.max(1) as f32)
        .unwrap_or(-1.0 / n.max(1) as f32);

    // Position of first bash (normalized)
    f[93] = steps
        .iter()
        .position(|s| s.bash_count > 0)
        .map(|p| p as f32 / n.max(1) as f32)
        .unwrap_or(-1.0 / n.max(1) as f32);

    // Fraction of steps with edits
    f[94] = steps.iter().filter(|s| s.edit_count > 0).count() as f32 / n.max(1) as f32;

    // Fraction of steps with reads only (no edit)
    f[95] = steps
        .iter()
        .filter(|s| (s.read_count + s.glob_count + s.grep_count) > 0 && s.edit_count == 0)
        .count() as f32
        / n.max(1) as f32;

    f
}
