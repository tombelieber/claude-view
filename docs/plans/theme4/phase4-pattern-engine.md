---
status: pending
date: 2026-02-05
phase: 4
theme: "Theme 4: Chat Insights & Pattern Discovery"
dependencies: [phase1-foundation]
parallelizable_with: [phase2-classification, phase3-system-page]
---

# Phase 4: Pattern Detection Engine

> **Goal:** Build a pattern detection engine that analyzes user behavior across sessions and generates actionable insights with statistical confidence.

## Overview

The Pattern Detection Engine is the analytical core of Theme 4. It processes session data to identify patterns in user behavior, prompt effectiveness, temporal trends, and workflow efficiency. Each pattern is scored for impact and converted into human-readable insights.

### Key Responsibilities

1. **Pattern Calculation** - Execute SQL queries and compute metrics for 60+ pattern types
2. **Impact Scoring** - Rank patterns by effect size, sample size, and actionability
3. **Insight Generation** - Convert raw patterns into natural language recommendations
4. **API Serving** - Expose patterns via `GET /api/insights` with caching

### Data Flow

```
Session Data (SQLite)
       |
       v
Pattern Calculators (60+ functions)
       |
       v
Impact Scoring Algorithm
       |
       v
Template-Based Insight Generator
       |
       v
GET /api/insights Response
```

---

## Pattern Catalog (60+ Patterns)

### Category 1: Prompt Patterns (10)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| P01 | `prompt_length` | Avg words per user prompt vs re-edit rate | n >= 50 sessions | "{optimal_range} word prompts have {improvement}% better first-attempt rate" |
| P02 | `question_vs_command` | Compare re-edit rate: prompts ending with ? vs imperative | n >= 30 each | "Commands outperform questions by {diff}%" |
| P03 | `specificity_score` | Prompts containing file paths vs not | n >= 20 each | "Prompts with file paths have {diff}% fewer re-edits" |
| P04 | `context_given` | Prompts referencing existing code (Read before prompt) | n >= 30 sessions | "Prompts referencing existing code succeed {multiplier}x more" |
| P05 | `constraint_clarity` | Prompts with explicit constraints ("must", "should not") | n >= 30 sessions | "Explicit constraints reduce re-edits by {diff}%" |
| P06 | `example_inclusion` | Prompts containing code examples | n >= 20 sessions | "Prompts with examples have {diff}% higher success" |
| P07 | `negative_constraints` | Prompts with "don't", "avoid", "not" | n >= 20 sessions | "'Don't use X' prompts have {diff}% higher re-edit rate" |
| P08 | `multi_step_vs_atomic` | Single-task vs multi-task prompts (detect "and then", numbered lists) | n >= 30 each | "Single-task prompts outperform multi-task by {diff}%" |
| P09 | `correction_language` | Prompts containing correction words ("wrong", "fix", "that's not") | n >= 30 sessions | "Your corrections average {count} per session" |
| P10 | `followup_depth` | Re-edit rate vs session turn count | n >= 50 sessions | "Sessions with >{threshold} follow-ups have diminishing returns" |

#### SQL Queries for Prompt Patterns

```sql
-- P01: prompt_length buckets
WITH prompt_stats AS (
  SELECT
    session_id,
    CASE
      WHEN avg_prompt_words < 50 THEN 'short'
      WHEN avg_prompt_words BETWEEN 50 AND 150 THEN 'medium'
      ELSE 'long'
    END as length_bucket,
    reedit_rate
  FROM (
    SELECT
      id as session_id,
      -- Approximate word count from token estimate
      (total_input_tokens / user_prompt_count) / 1.3 as avg_prompt_words,
      CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0) as reedit_rate
    FROM sessions
    WHERE user_prompt_count > 0 AND files_edited_count > 0
      AND last_message_at >= ?1  -- time range filter
  )
)
SELECT
  length_bucket,
  COUNT(*) as session_count,
  AVG(reedit_rate) as avg_reedit_rate
FROM prompt_stats
GROUP BY length_bucket
HAVING COUNT(*) >= 20;

-- P02: question_vs_command (requires prompt text analysis in turns table)
WITH prompt_types AS (
  SELECT
    t.session_id,
    CASE
      WHEN t.content LIKE '%?' THEN 'question'
      ELSE 'command'
    END as prompt_type,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE t.role = 'user'
    AND s.files_edited_count > 0
    AND s.last_message_at >= ?1
)
SELECT
  prompt_type,
  COUNT(DISTINCT session_id) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM prompt_types
GROUP BY prompt_type
HAVING COUNT(DISTINCT session_id) >= 30;

-- P03: specificity_score (prompts with file paths)
WITH prompt_specificity AS (
  SELECT
    t.session_id,
    CASE
      WHEN t.content LIKE '%/%' OR t.content LIKE '%.ts%' OR t.content LIKE '%.rs%'
           OR t.content LIKE '%.py%' OR t.content LIKE '%.js%' THEN 'with_paths'
      ELSE 'no_paths'
    END as specificity,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE t.role = 'user'
    AND s.files_edited_count > 0
    AND s.last_message_at >= ?1
)
SELECT
  specificity,
  COUNT(DISTINCT session_id) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM prompt_specificity
GROUP BY specificity
HAVING COUNT(DISTINCT session_id) >= 20;

-- P04: context_given (Read before prompt)
SELECT
  CASE WHEN files_read_count > 0 THEN 'with_context' ELSE 'no_context' END as context_type,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY context_type;

-- P05: example_inclusion (prompts with code examples - backticks or indentation)
WITH prompt_examples AS (
  SELECT
    t.session_id,
    CASE
      WHEN t.content LIKE '%```%' OR t.content LIKE '%`%`%' THEN 'with_examples'
      ELSE 'no_examples'
    END as has_example,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE t.role = 'user'
    AND s.files_edited_count > 0
    AND s.last_message_at >= ?1
)
SELECT
  has_example,
  COUNT(DISTINCT session_id) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM prompt_examples
GROUP BY has_example
HAVING COUNT(DISTINCT session_id) >= 20;

-- P10: followup_depth (turn count vs reedit rate)
SELECT
  CASE
    WHEN turn_count <= 5 THEN '1-5'
    WHEN turn_count <= 10 THEN '6-10'
    WHEN turn_count <= 15 THEN '11-15'
    ELSE '16+'
  END as turn_bucket,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY turn_bucket
ORDER BY turn_bucket;
```

### Category 2: Session Patterns (8)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| S01 | `optimal_duration` | Duration buckets vs files_edited_count/duration | n >= 50 sessions | "{min}-{max} min sessions have best ROI" |
| S02 | `turn_count_sweet_spot` | Turn count vs reedit rate | n >= 50 sessions | "{min}-{max} turns optimal" |
| S03 | `warmup_effect` | First prompt reedit rate vs subsequent | n >= 100 prompts | "First prompt has {diff}% higher re-edit rate" |
| S04 | `fatigue_signal` | Re-edit rate for turns > 12 vs earlier | n >= 50 sessions | "Re-edit rate increases {diff}% after turn {threshold}" |
| S05 | `session_restart_benefit` | Compare continued vs fresh sessions (same project, same day) | n >= 20 pairs | "Fresh sessions outperform continued by {diff}%" |
| S06 | `context_window_usage` | Sessions with high token usage vs reedit | n >= 30 sessions | "Sessions near context limit have {multiplier}x re-edit" |
| S07 | `skill_activation_timing` | Skill usage in first vs second half of session | n >= 30 sessions | "{skill} at start = {diff}% better outcomes" |
| S08 | `file_count_correlation` | Files edited count vs reedit rate | n >= 50 sessions | ">{threshold} files = {diff}% higher re-edit" |

#### SQL Queries for Session Patterns

```sql
-- S01: optimal_duration buckets
SELECT
  CASE
    WHEN duration_seconds < 900 THEN '<15min'
    WHEN duration_seconds < 2700 THEN '15-45min'
    WHEN duration_seconds < 5400 THEN '45-90min'
    ELSE '>90min'
  END as duration_bucket,
  COUNT(*) as session_count,
  AVG(CAST(files_edited_count AS REAL) / (duration_seconds / 60.0)) as edits_per_minute,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE duration_seconds > 0 AND files_edited_count > 0 AND last_message_at >= ?1
GROUP BY duration_bucket
HAVING COUNT(*) >= 10;

-- S02: turn_count_sweet_spot
SELECT
  CASE
    WHEN turn_count <= 5 THEN '1-5'
    WHEN turn_count <= 8 THEN '6-8'
    WHEN turn_count <= 12 THEN '9-12'
    WHEN turn_count <= 15 THEN '13-15'
    ELSE '16+'
  END as turn_bucket,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY turn_bucket;

-- S08: file_count_correlation
SELECT
  CASE
    WHEN files_edited_count <= 3 THEN '1-3'
    WHEN files_edited_count <= 7 THEN '4-7'
    WHEN files_edited_count <= 10 THEN '8-10'
    ELSE '11+'
  END as file_bucket,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY file_bucket;
```

### Category 3: Temporal Patterns (7)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| T01 | `time_of_day` | Hour of day vs efficiency metrics | n >= 100 sessions | "{start}-{end} is {diff}% more efficient than {worst_period}" |
| T02 | `day_of_week` | Weekday vs reedit rate | n >= 100 sessions | "{best_day} most productive; {worst_day} highest re-edit" |
| T03 | `week_patterns` | Mon-Tue vs Thu-Fri work patterns | n >= 50 sessions | "Week starts: planning; week ends: bug fixes" |
| T04 | `sprint_timing` | Detect sprint patterns (2-week cycles) | n >= 8 weeks | "End-of-sprint has {multiplier}x re-edit rate" |
| T05 | `break_impact` | Sessions after 2+ day gap vs continuous | n >= 20 each | "{days}+ day break = {diff}% warmup penalty" |
| T06 | `consecutive_sessions` | Multiple sessions same day vs spread | n >= 30 days | "{count}+ consecutive sessions = {diff}% efficiency drop" |
| T07 | `monthly_trends` | Month-over-month efficiency comparison | n >= 3 months | "{diff}% month-over-month improvement" |

#### SQL Queries for Temporal Patterns

```sql
-- T01: time_of_day efficiency
SELECT
  CASE
    WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 6 AND 11 THEN 'morning'
    WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 12 AND 17 THEN 'afternoon'
    WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 18 AND 22 THEN 'evening'
    ELSE 'night'
  END as time_slot,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
  AVG(CAST(files_edited_count AS REAL) / (duration_seconds / 60.0)) as edits_per_minute
FROM sessions
WHERE first_message_at IS NOT NULL
  AND files_edited_count > 0
  AND duration_seconds > 0
  AND last_message_at >= ?1
GROUP BY time_slot
HAVING COUNT(*) >= 10;

-- T02: day_of_week
SELECT
  strftime('%w', datetime(first_message_at, 'unixepoch', 'localtime')) as day_of_week,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE first_message_at IS NOT NULL AND files_edited_count > 0 AND last_message_at >= ?1
GROUP BY day_of_week;

-- T05: break_impact
WITH session_gaps AS (
  SELECT
    id,
    project_id,
    first_message_at,
    LAG(last_message_at) OVER (PARTITION BY project_id ORDER BY first_message_at) as prev_session_end,
    reedited_files_count,
    files_edited_count
  FROM sessions
  WHERE first_message_at IS NOT NULL AND files_edited_count > 0
)
SELECT
  CASE
    WHEN (first_message_at - prev_session_end) / 86400 >= 2 THEN 'after_break'
    ELSE 'continuous'
  END as session_type,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM session_gaps
WHERE prev_session_end IS NOT NULL AND first_message_at >= ?1
GROUP BY session_type;

-- T07: monthly_trends
SELECT
  strftime('%Y-%m', datetime(first_message_at, 'unixepoch')) as month,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
  AVG(CAST(files_edited_count AS REAL) / (duration_seconds / 60.0)) as avg_edit_velocity
FROM sessions
WHERE first_message_at IS NOT NULL
  AND files_edited_count > 0
  AND duration_seconds > 0
GROUP BY month
ORDER BY month;
```

### Category 4: Workflow Patterns (8)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| W01 | `skill_sequences` | Common skill orderings and their outcomes | n >= 30 sessions | "{skill1} -> {skill2} -> {skill3} = highest success" |
| W02 | `category_transitions` | L1 category sequences (when classification available) | n >= 50 sessions | "Bug fix -> Refactor is common pattern" |
| W03 | `planning_to_execution` | Sessions with early Read vs immediate Edit | n >= 50 sessions | "Planning phase = {diff}% better outcomes" |
| W04 | `test_first_correlation` | Sessions with test file edits before impl | n >= 30 sessions | "Tests before impl = {diff}% fewer re-edits" |
| W05 | `commit_frequency` | Frequent commits vs end-of-session commits | n >= 50 sessions | "Every {interval} vs end = {diff}% less lost work" |
| W06 | `branch_discipline` | Feature branch vs main branch sessions | n >= 30 each | "Feature branch sessions have {diff}% lower re-edit" |
| W07 | `read_before_write` | Read file count before first edit | n >= 50 sessions | "Read files first = {diff}% better edits" |
| W08 | `exploration_vs_execution` | High read/low edit sessions preceding productive sessions | n >= 20 pairs | "Pure exploration precedes {diff}% of best work" |

#### SQL Queries for Workflow Patterns

```sql
-- W01: skill_sequences (common skill orderings)
-- Note: Requires skill_usage table populated from JSONL parsing
WITH skill_pairs AS (
  SELECT
    s1.session_id,
    s1.skill_name as first_skill,
    s2.skill_name as second_skill,
    sess.reedited_files_count,
    sess.files_edited_count
  FROM skill_usage s1
  JOIN skill_usage s2 ON s1.session_id = s2.session_id AND s1.used_at < s2.used_at
  JOIN sessions sess ON s1.session_id = sess.id
  WHERE sess.files_edited_count > 0
    AND sess.last_message_at >= ?1
    AND NOT EXISTS (
      SELECT 1 FROM skill_usage s3
      WHERE s3.session_id = s1.session_id
        AND s3.used_at > s1.used_at
        AND s3.used_at < s2.used_at
    )
)
SELECT
  first_skill || ' -> ' || second_skill as sequence,
  COUNT(*) as occurrence_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM skill_pairs
GROUP BY first_skill, second_skill
HAVING COUNT(*) >= 10
ORDER BY occurrence_count DESC
LIMIT 20;

-- W03: planning_to_execution (read-to-edit ratio)
SELECT
  CASE
    WHEN files_read_count > files_edited_count * 2 THEN 'heavy_planning'
    WHEN files_read_count > files_edited_count THEN 'some_planning'
    ELSE 'execution_focused'
  END as planning_style,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY planning_style
HAVING COUNT(*) >= 10;

-- W04: test_first_correlation (detect test files in edited files)
SELECT
  CASE
    WHEN files_edited LIKE '%test%' OR files_edited LIKE '%spec%' THEN 'has_tests'
    ELSE 'no_tests'
  END as test_pattern,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY test_pattern;

-- W06: branch_discipline
SELECT
  CASE
    WHEN git_branch = 'main' OR git_branch = 'master' THEN 'main_branch'
    ELSE 'feature_branch'
  END as branch_type,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE git_branch IS NOT NULL AND files_edited_count > 0 AND last_message_at >= ?1
GROUP BY branch_type;

-- W07: read_before_write ratio buckets
SELECT
  CASE
    WHEN files_read_count = 0 THEN 'no_reads'
    WHEN CAST(files_read_count AS REAL) / files_edited_count < 1 THEN 'low_reads'
    WHEN CAST(files_read_count AS REAL) / files_edited_count < 3 THEN 'moderate_reads'
    ELSE 'high_reads'
  END as read_pattern,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY read_pattern;
```

### Category 5: Model Patterns (5)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| M01 | `model_task_fit` | Model family vs category reedit rate | n >= 30 per model | "{model} for {task_type} has {diff}% lower re-edit" |
| M02 | `model_switching` | Mid-session model switches and outcomes | n >= 20 switches | "{pct}% mid-session switches, usually {outcome}" |
| M03 | `cost_quality_tradeoff` | Haiku vs Sonnet vs Opus for similar tasks | n >= 30 per model | "{cheaper_model} for {task} costs {multiplier}x less, same success" |
| M04 | `model_by_category` | Model usage per L1 category | n >= 50 sessions | "{model} {task} has {diff}% lower re-edit" |
| M05 | `model_by_complexity` | Model vs file count correlation | n >= 50 sessions | "High complexity: {best_model} wins by {diff}%" |

#### SQL Queries for Model Patterns

```sql
-- M01: model_task_fit (requires joins with turns)
WITH session_models AS (
  SELECT
    s.id as session_id,
    COALESCE(
      (SELECT model_id FROM turns t
       WHERE t.session_id = s.id
       GROUP BY model_id
       ORDER BY COUNT(*) DESC
       LIMIT 1),
      'unknown'
    ) as primary_model,
    s.reedited_files_count,
    s.files_edited_count
  FROM sessions s
  WHERE s.files_edited_count > 0 AND s.last_message_at >= ?1
)
SELECT
  CASE
    WHEN primary_model LIKE '%opus%' THEN 'opus'
    WHEN primary_model LIKE '%sonnet%' THEN 'sonnet'
    WHEN primary_model LIKE '%haiku%' THEN 'haiku'
    ELSE 'other'
  END as model_family,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM session_models
GROUP BY model_family
HAVING COUNT(*) >= 10;

-- M05: model_by_complexity (file count as complexity proxy)
WITH session_models AS (
  SELECT
    s.id,
    COALESCE(
      (SELECT model_id FROM turns t
       WHERE t.session_id = s.id
       GROUP BY model_id
       ORDER BY COUNT(*) DESC
       LIMIT 1),
      'unknown'
    ) as primary_model,
    s.files_edited_count,
    s.reedited_files_count,
    CASE
      WHEN s.files_edited_count <= 3 THEN 'low'
      WHEN s.files_edited_count <= 7 THEN 'medium'
      ELSE 'high'
    END as complexity
  FROM sessions s
  WHERE s.files_edited_count > 0 AND s.last_message_at >= ?1
)
SELECT
  complexity,
  CASE
    WHEN primary_model LIKE '%opus%' THEN 'opus'
    WHEN primary_model LIKE '%sonnet%' THEN 'sonnet'
    WHEN primary_model LIKE '%haiku%' THEN 'haiku'
    ELSE 'other'
  END as model_family,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM session_models
GROUP BY complexity, model_family
HAVING COUNT(*) >= 5;
```

### Category 6: Codebase Patterns (7)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| C01 | `language_efficiency` | File extension vs reedit rate | n >= 30 per language | "{lang} is {diff}% more efficient than {baseline}" |
| C02 | `file_type_patterns` | Test vs config vs source files | n >= 30 per type | "Test files: lowest re-edit; config: highest" |
| C03 | `project_complexity` | Per-project reedit rates | n >= 20 sessions/project | "Project {name} has {multiplier}x re-edit rate" |
| C04 | `new_vs_existing` | New file creation vs modification | n >= 50 sessions | "New files have {diff}% better outcomes than modifying" |
| C05 | `directory_hotspots` | Directory path vs edit frequency | n >= 100 file edits | "{dir} has highest AI contribution" |
| C06 | `dependency_patterns` | Config file edits (package.json, etc.) | n >= 30 sessions | "Dependency changes = {diff}% higher re-edit" |
| C07 | `monorepo_patterns` | Cross-package edits in monorepos | n >= 20 sessions | "Cross-package edits = {multiplier}x re-edit" |

#### SQL Queries for Codebase Patterns

```sql
-- C01: language_efficiency (extract extension from files_edited JSON)
-- Note: This requires JSON parsing in application layer

-- C03: project_complexity
SELECT
  project_display_name as project,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
  SUM(files_edited_count) as total_files_edited
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY project_id, project_display_name
HAVING COUNT(*) >= 10
ORDER BY avg_reedit_rate DESC;

-- C04: new_vs_existing (detect via Write vs Edit tool counts)
SELECT
  CASE
    WHEN tool_counts_write > tool_counts_edit THEN 'mostly_new'
    WHEN tool_counts_write > 0 THEN 'mixed'
    ELSE 'mostly_modify'
  END as edit_pattern,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY edit_pattern;
```

### Category 7: Outcome Patterns (5)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| O01 | `commit_rate_by_category` | Sessions resulting in commits per category | n >= 50 sessions | "Feature: {pct}%; Bug fixes: {pct}%" |
| O02 | `abandoned_sessions` | Sessions with 0 commits | n >= 100 sessions | "{pct}% produce no commits - exploration sessions" |
| O03 | `revert_correlation` | Session duration vs revert likelihood | n >= 50 sessions | ">{threshold}hr sessions = {multiplier}x higher revert" |
| O04 | `pr_success` | Skill usage correlation with merge success | n >= 30 PRs | "{skill} sessions = PRs merged {multiplier}x faster" |
| O05 | `bug_recurrence` | AI-fixed bugs recurring vs manual fixes | n >= 30 bugs | "AI-fixed bugs: same recurrence as manual" |

#### SQL Queries for Outcome Patterns

```sql
-- O01: commit_rate (requires classification data)
SELECT
  CASE WHEN commit_count > 0 THEN 'committed' ELSE 'no_commit' END as outcome,
  COUNT(*) as session_count
FROM sessions
WHERE last_message_at >= ?1
GROUP BY outcome;

-- O02: abandoned_sessions
SELECT
  CASE
    WHEN commit_count = 0 AND duration_seconds > 300 THEN 'abandoned'
    WHEN commit_count = 0 AND duration_seconds <= 300 THEN 'quick_lookup'
    ELSE 'productive'
  END as session_outcome,
  COUNT(*) as session_count,
  AVG(duration_seconds / 60.0) as avg_duration_min
FROM sessions
WHERE last_message_at >= ?1
GROUP BY session_outcome;

-- O03: revert_correlation (duration buckets)
SELECT
  CASE
    WHEN duration_seconds < 3600 THEN '<1hr'
    WHEN duration_seconds < 7200 THEN '1-2hr'
    ELSE '>2hr'
  END as duration_bucket,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY duration_bucket;
```

### Category 8: Behavioral Patterns (7)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| B01 | `retry_patterns` | Average re-edits before success | n >= 50 sessions | "{avg} retries before rephrasing" |
| B02 | `escalation_patterns` | Model upgrade after failures | n >= 30 switches | "After {count} failures, switch to {model}" |
| B03 | `abandonment_triggers` | What precedes abandoned sessions | n >= 30 abandoned | "Abandon after {count}+ re-edits" |
| B04 | `copy_paste_frequency` | External code paste detection | n >= 50 sessions | "{pct}% paste external code - high success" |
| B05 | `screenshot_usage` | Sessions with image tools | n >= 20 sessions | "Screenshots = {diff}% higher success" |
| B06 | `url_sharing` | Sessions with URL references | n >= 30 sessions | "Docs URLs improve API work by {diff}%" |
| B07 | `frustration_signals` | Prompt length increase pattern | n >= 50 sessions | "Prompt length {multiplier}x when stuck" |

#### SQL Queries for Behavioral Patterns

```sql
-- B01: retry_patterns
SELECT
  AVG(reedited_files_count) as avg_reedits,
  AVG(CASE WHEN reedited_files_count > 0 THEN reedited_files_count ELSE NULL END) as avg_reedits_when_any,
  COUNT(CASE WHEN reedited_files_count > 0 THEN 1 END) as sessions_with_reedits,
  COUNT(*) as total_sessions
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1;

-- B02: escalation_patterns (model upgrade after failures)
WITH session_models AS (
  SELECT
    t.session_id,
    t.model_id,
    t.turn_number,
    LAG(t.model_id) OVER (PARTITION BY t.session_id ORDER BY t.turn_number) as prev_model,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE s.last_message_at >= ?1
)
SELECT
  CASE
    WHEN prev_model LIKE '%haiku%' AND model_id LIKE '%sonnet%' THEN 'haiku_to_sonnet'
    WHEN prev_model LIKE '%sonnet%' AND model_id LIKE '%opus%' THEN 'sonnet_to_opus'
    WHEN prev_model LIKE '%haiku%' AND model_id LIKE '%opus%' THEN 'haiku_to_opus'
    ELSE 'other'
  END as escalation_type,
  COUNT(*) as switch_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_after
FROM session_models
WHERE prev_model IS NOT NULL AND prev_model != model_id
GROUP BY escalation_type
HAVING COUNT(*) >= 10;

-- B03: abandonment_triggers
SELECT
  CASE
    WHEN reedited_files_count = 0 THEN '0_reedits'
    WHEN reedited_files_count <= 2 THEN '1-2_reedits'
    WHEN reedited_files_count <= 5 THEN '3-5_reedits'
    ELSE '6+_reedits'
  END as reedit_bucket,
  COUNT(*) as session_count,
  SUM(CASE WHEN commit_count = 0 THEN 1 ELSE 0 END) as abandoned_count,
  CAST(SUM(CASE WHEN commit_count = 0 THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as abandon_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY reedit_bucket;

-- B04: copy_paste_frequency (detect external code paste via large user prompts)
WITH prompt_sizes AS (
  SELECT
    t.session_id,
    MAX(LENGTH(t.content)) as max_prompt_length,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE t.role = 'user'
    AND s.files_edited_count > 0
    AND s.last_message_at >= ?1
  GROUP BY t.session_id
)
SELECT
  CASE
    WHEN max_prompt_length > 2000 THEN 'large_paste'
    WHEN max_prompt_length > 500 THEN 'medium'
    ELSE 'small'
  END as paste_pattern,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM prompt_sizes
GROUP BY paste_pattern
HAVING COUNT(*) >= 10;

-- B05: screenshot_usage (sessions with image tool usage)
SELECT
  CASE
    WHEN tool_counts_screenshot > 0 OR tool_counts_image > 0 THEN 'with_images'
    ELSE 'no_images'
  END as image_usage,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM sessions
WHERE files_edited_count > 0 AND last_message_at >= ?1
GROUP BY image_usage
HAVING COUNT(*) >= 10;

-- B06: url_sharing (sessions with URL references in prompts)
WITH url_sessions AS (
  SELECT
    t.session_id,
    MAX(CASE WHEN t.content LIKE '%http://%' OR t.content LIKE '%https://%' THEN 1 ELSE 0 END) as has_url,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE t.role = 'user'
    AND s.files_edited_count > 0
    AND s.last_message_at >= ?1
  GROUP BY t.session_id
)
SELECT
  CASE WHEN has_url = 1 THEN 'with_urls' ELSE 'no_urls' END as url_pattern,
  COUNT(*) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM url_sessions
GROUP BY url_pattern
HAVING COUNT(*) >= 10;

-- B07: frustration_signals (prompt length increase within session)
WITH prompt_lengths AS (
  SELECT
    t.session_id,
    t.turn_number,
    LENGTH(t.content) as prompt_length,
    FIRST_VALUE(LENGTH(t.content)) OVER (PARTITION BY t.session_id ORDER BY t.turn_number) as first_length,
    LAST_VALUE(LENGTH(t.content)) OVER (PARTITION BY t.session_id ORDER BY t.turn_number
      ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) as last_length,
    s.reedited_files_count,
    s.files_edited_count
  FROM turns t
  JOIN sessions s ON t.session_id = s.id
  WHERE t.role = 'user'
    AND s.files_edited_count > 0
    AND s.last_message_at >= ?1
)
SELECT
  CASE
    WHEN last_length > first_length * 2 THEN 'frustration_likely'
    WHEN last_length > first_length * 1.5 THEN 'some_escalation'
    ELSE 'stable'
  END as frustration_signal,
  COUNT(DISTINCT session_id) as session_count,
  AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
FROM prompt_lengths
GROUP BY frustration_signal
HAVING COUNT(DISTINCT session_id) >= 10;
```

### Category 9: Comparative Patterns (3)

| ID | Pattern | Calculation | Threshold | Insight Template |
|----|---------|-------------|-----------|------------------|
| CP01 | `you_vs_baseline` | User's recent metrics vs 30-day baseline | n >= 30 sessions | "{diff}% more efficient than 30-day avg" |
| CP02 | `category_benchmarks` | Per-category percentile ranking | n >= 50 sessions | "Bug fix: top quartile; refactors need work" |
| CP03 | `skill_adoption_curve` | Skill usage impact over time | n >= 5 sessions with skill | "{skill} took {sessions} sessions to show benefits" |

#### SQL Queries for Comparative Patterns

```sql
-- CP01: you_vs_baseline (7-day vs 30-day comparison)
WITH baseline AS (
  SELECT
    AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as baseline_reedit,
    AVG(CAST(files_edited_count AS REAL) / (duration_seconds / 60.0)) as baseline_velocity
  FROM sessions
  WHERE files_edited_count > 0
    AND duration_seconds > 0
    AND last_message_at >= ?1 - 2592000  -- 30 days
    AND last_message_at < ?1 - 604800    -- before last 7 days
),
recent AS (
  SELECT
    AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as recent_reedit,
    AVG(CAST(files_edited_count AS REAL) / (duration_seconds / 60.0)) as recent_velocity
  FROM sessions
  WHERE files_edited_count > 0
    AND duration_seconds > 0
    AND last_message_at >= ?1 - 604800  -- last 7 days
)
SELECT
  b.baseline_reedit,
  r.recent_reedit,
  b.baseline_velocity,
  r.recent_velocity,
  CASE WHEN b.baseline_reedit > 0
    THEN ((b.baseline_reedit - r.recent_reedit) / b.baseline_reedit) * 100
    ELSE NULL END as reedit_improvement_pct,
  CASE WHEN b.baseline_velocity > 0
    THEN ((r.recent_velocity - b.baseline_velocity) / b.baseline_velocity) * 100
    ELSE NULL END as velocity_improvement_pct
FROM baseline b, recent r;
```

---

## Impact Scoring Algorithm

Each pattern is scored 0.0-1.0 based on three factors:

### 1. Effect Size (40% weight)

Measures how much improvement the pattern suggests.

```rust
fn calculate_effect_size(better: f64, baseline: f64) -> f64 {
    if baseline == 0.0 {
        return 0.0;
    }

    let relative_diff = (baseline - better).abs() / baseline;

    // Cohen's d-like interpretation:
    // < 10%  -> small effect (0.2)
    // 10-25% -> medium effect (0.5)
    // > 25%  -> large effect (0.8)
    match relative_diff {
        d if d < 0.10 => d * 2.0,      // 0.0 - 0.2
        d if d < 0.25 => 0.2 + (d - 0.10) * 2.0,  // 0.2 - 0.5
        d if d < 0.50 => 0.5 + (d - 0.25) * 1.2,  // 0.5 - 0.8
        _ => 0.8 + (relative_diff - 0.50).min(0.2),  // 0.8 - 1.0
    }
}
```

### 2. Sample Size (30% weight)

Statistical confidence based on observation count.

```rust
fn calculate_sample_confidence(n: u32, threshold: u32) -> f64 {
    if n < threshold {
        return 0.0;  // Below minimum, pattern not valid
    }

    // Logarithmic scaling for sample size
    // threshold -> 0.5
    // 2x threshold -> 0.75
    // 5x threshold -> 0.9
    // 10x+ threshold -> 1.0
    let ratio = n as f64 / threshold as f64;
    1.0 - (1.0 / (1.0 + ratio.ln().max(0.0)))
}
```

### 3. Actionability (30% weight)

How easily the user can act on this insight.

```rust
#[derive(Clone, Copy)]
enum Actionability {
    /// User can change immediately (e.g., prompt length)
    Immediate = 100,
    /// User can change with some effort (e.g., skill usage)
    Moderate = 70,
    /// Awareness-only, hard to change (e.g., time of day)
    Awareness = 40,
    /// Informational, no clear action (e.g., historical trend)
    Informational = 20,
}

fn actionability_score(pattern: &Pattern) -> f64 {
    pattern.actionability as u8 as f64 / 100.0
}
```

### Combined Score

```rust
pub struct PatternScore {
    pub effect_size: f64,      // 0.0 - 1.0
    pub sample_confidence: f64, // 0.0 - 1.0
    pub actionability: f64,     // 0.0 - 1.0
    pub combined: f64,          // 0.0 - 1.0
}

impl PatternScore {
    pub fn calculate(effect: f64, sample: f64, action: f64) -> Self {
        let combined = effect * 0.4 + sample * 0.3 + action * 0.3;
        Self {
            effect_size: effect,
            sample_confidence: sample,
            actionability: action,
            combined,
        }
    }

    /// Returns the impact tier based on combined score
    pub fn tier(&self) -> &'static str {
        match self.combined {
            c if c >= 0.7 => "high",
            c if c >= 0.4 => "medium",
            _ => "observation",
        }
    }
}

/// Calculate pattern score from components
/// Used by pattern implementations to score their results
pub fn calculate_pattern_score(
    relative_improvement: f64,  // e.g., 0.25 for 25% improvement
    sample_size: u32,
    min_sample_size: u32,
    actionability: Actionability,
) -> PatternScore {
    let effect = calculate_effect_size(relative_improvement, 1.0);
    let sample = calculate_sample_confidence(sample_size, min_sample_size);
    let action = actionability_score(actionability);

    PatternScore::calculate(effect, sample, action)
}

fn actionability_score(actionability: Actionability) -> f64 {
    match actionability {
        Actionability::Immediate => 1.0,
        Actionability::Moderate => 0.7,
        Actionability::Awareness => 0.4,
        Actionability::Informational => 0.2,
    }
}
```

### Impact Tiers

| Tier | Score Range | Description |
|------|-------------|-------------|
| High | 0.7 - 1.0 | Significant, well-supported, actionable |
| Medium | 0.4 - 0.7 | Notable pattern, moderate confidence |
| Observation | 0.0 - 0.4 | Interesting but low confidence or effect |

---

## Insight Text Generation

### Template System

Each pattern has templates for generating human-readable insights.

```rust
pub struct InsightTemplate {
    pub pattern_id: &'static str,
    pub title_template: &'static str,
    pub body_template: &'static str,
    pub recommendation_template: Option<&'static str>,
}

pub const TEMPLATES: &[InsightTemplate] = &[
    InsightTemplate {
        pattern_id: "P01",
        title_template: "Optimal Prompt Length",
        body_template: "{optimal_range} word prompts have {improvement:.0}% better first-attempt success rate than {worst_range} word prompts.",
        recommendation_template: Some("Try keeping prompts between {min_words} and {max_words} words for best results."),
    },
    InsightTemplate {
        pattern_id: "S01",
        title_template: "Session Duration Sweet Spot",
        body_template: "Your {optimal_duration} sessions produce {improvement:.0}% more edits per minute than {worst_duration} sessions.",
        recommendation_template: Some("Consider breaking longer sessions into {optimal_duration} chunks."),
    },
    InsightTemplate {
        pattern_id: "T01",
        title_template: "Peak Productivity Hours",
        body_template: "You're {improvement:.0}% more efficient during {best_time} compared to {worst_time}.",
        recommendation_template: Some("Schedule complex tasks for {best_time} when possible."),
    },
    // ... 57 more templates
];
```

### Template Variable Substitution

```rust
use std::collections::HashMap;

pub fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        // Handle format specifiers like {improvement:.0}
        let patterns = [
            format!("{{{}}}", key),
            format!("{{{}:.0}}", key),
            format!("{{{}:.1}}", key),
            format!("{{{}:.2}}", key),
        ];
        for pattern in &patterns {
            if result.contains(pattern) {
                let formatted = if pattern.contains(".0") {
                    format!("{:.0}", value.parse::<f64>().unwrap_or(0.0))
                } else if pattern.contains(".1") {
                    format!("{:.1}", value.parse::<f64>().unwrap_or(0.0))
                } else if pattern.contains(".2") {
                    format!("{:.2}", value.parse::<f64>().unwrap_or(0.0))
                } else {
                    value.clone()
                };
                result = result.replace(pattern, &formatted);
            }
        }
    }
    result
}
```

### Generated Insight Structure

```rust
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct GeneratedInsight {
    pub pattern_id: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub recommendation: Option<String>,
    pub impact_score: f64,
    pub impact_tier: String,  // "high" | "medium" | "observation"
    pub evidence: InsightEvidence,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightEvidence {
    pub sample_size: u32,
    pub time_range_days: u32,
    pub comparison_values: HashMap<String, f64>,
}
```

---

## API Specification

### GET /api/insights

Returns computed insights for the user.

#### Query Parameters

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `from` | unix timestamp | 30 days ago | Period start |
| `to` | unix timestamp | now | Period end |
| `min_impact` | f64 (0.0-1.0) | 0.3 | Minimum impact score |
| `categories` | comma-separated | all | Filter pattern categories |
| `limit` | u32 | 50 | Max patterns to return |

#### Response

```typescript
interface InsightsResponse {
  // Hero insight (highest impact)
  topInsight: {
    patternId: string;
    category: string;
    title: string;
    body: string;
    recommendation: string | null;
    impactScore: number;
    impactTier: 'high' | 'medium' | 'observation';
    evidence: {
      sampleSize: number;
      timeRangeDays: number;
      comparisonValues: Record<string, number>;
    };
  } | null;

  // Overview stats
  overview: {
    workBreakdown: {
      totalSessions: number;
      withCommits: number;
      exploration: number;
      avgSessionMinutes: number;
    };
    efficiency: {
      avgReeditRate: number;
      avgEditVelocity: number;
      trend: 'improving' | 'stable' | 'declining';
      trendPct: number;
    };
    bestTime: {
      dayOfWeek: string;
      timeSlot: string;
      improvementPct: number;
    };
  };

  // Patterns grouped by impact tier
  patterns: {
    high: GeneratedInsight[];
    medium: GeneratedInsight[];
    observations: GeneratedInsight[];
  };

  // Classification coverage (for Phase 6 Categories tab)
  classificationStatus: {
    classified: number;
    total: number;
    pendingClassification: number;
    classificationPct: number;
  };

  // Metadata
  meta: {
    computedAt: number;
    timeRangeStart: number;
    timeRangeEnd: number;
    patternsEvaluated: number;
    patternsReturned: number;
  };
}
```

### Rust InsightsResponse Struct

```rust
// crates/server/src/routes/insights.rs

use serde::Serialize;
use ts_rs::TS;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightsResponse {
    pub top_insight: Option<GeneratedInsight>,
    pub overview: InsightsOverview,
    pub patterns: PatternGroups,
    pub classification_status: ClassificationStatus,
    pub meta: InsightsMeta,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightsOverview {
    pub work_breakdown: WorkBreakdown,
    pub efficiency: EfficiencyStats,
    pub best_time: BestTimeStats,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct WorkBreakdown {
    pub total_sessions: u32,
    pub with_commits: u32,
    pub exploration: u32,
    pub avg_session_minutes: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyStats {
    pub avg_reedit_rate: f64,
    pub avg_edit_velocity: f64,
    pub trend: String,  // "improving" | "stable" | "declining"
    pub trend_pct: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BestTimeStats {
    pub day_of_week: String,
    pub time_slot: String,
    pub improvement_pct: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PatternGroups {
    pub high: Vec<GeneratedInsight>,
    pub medium: Vec<GeneratedInsight>,
    pub observations: Vec<GeneratedInsight>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassificationStatus {
    pub classified: u32,
    pub total: u32,
    pub pending_classification: u32,
    pub classification_pct: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightsMeta {
    pub computed_at: i64,
    pub time_range_start: i64,
    pub time_range_end: i64,
    pub patterns_evaluated: u32,
    pub patterns_returned: u32,
}
```

### Helper Function Implementations

```rust
// crates/server/src/routes/insights.rs

/// Calculate overview statistics for the insights response
async fn calculate_overview(
    pool: &SqlitePool,
    from_ts: i64,
    to_ts: i64,
) -> Result<InsightsOverview, PatternError> {
    // Work breakdown query
    #[derive(sqlx::FromRow)]
    struct WorkStats {
        total_sessions: i64,
        with_commits: i64,
        exploration: i64,
        avg_duration_seconds: f64,
    }

    let work: WorkStats = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total_sessions,
            SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END) as with_commits,
            SUM(CASE WHEN commit_count = 0 AND files_edited_count = 0 THEN 1 ELSE 0 END) as exploration,
            AVG(duration_seconds) as avg_duration_seconds
        FROM sessions
        WHERE last_message_at >= ?1 AND last_message_at <= ?2
        "#
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    // Efficiency stats query
    #[derive(sqlx::FromRow)]
    struct EffStats {
        avg_reedit_rate: Option<f64>,
        avg_edit_velocity: Option<f64>,
    }

    let eff: EffStats = sqlx::query_as(
        r#"
        SELECT
            AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
            AVG(CAST(files_edited_count AS REAL) / NULLIF(duration_seconds / 60.0, 0)) as avg_edit_velocity
        FROM sessions
        WHERE files_edited_count > 0
            AND duration_seconds > 0
            AND last_message_at >= ?1 AND last_message_at <= ?2
        "#
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    // Trend calculation: compare recent 7 days vs previous period
    let week_ago = to_ts - 7 * 86400;
    let (trend, trend_pct) = calculate_efficiency_trend(pool, from_ts, week_ago, to_ts).await?;

    // Best time query
    #[derive(sqlx::FromRow)]
    struct TimeSlotStats {
        day_of_week: String,
        time_slot: String,
        avg_reedit_rate: f64,
    }

    let best_time: Option<TimeSlotStats> = sqlx::query_as(
        r#"
        SELECT
            CASE CAST(strftime('%w', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER)
                WHEN 0 THEN 'Sunday' WHEN 1 THEN 'Monday' WHEN 2 THEN 'Tuesday'
                WHEN 3 THEN 'Wednesday' WHEN 4 THEN 'Thursday' WHEN 5 THEN 'Friday'
                ELSE 'Saturday'
            END as day_of_week,
            CASE
                WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 6 AND 11 THEN 'morning'
                WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 12 AND 17 THEN 'afternoon'
                WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 18 AND 22 THEN 'evening'
                ELSE 'night'
            END as time_slot,
            AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
        FROM sessions
        WHERE first_message_at IS NOT NULL
            AND files_edited_count > 0
            AND last_message_at >= ?1 AND last_message_at <= ?2
        GROUP BY day_of_week, time_slot
        ORDER BY avg_reedit_rate ASC
        LIMIT 1
        "#
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_optional(pool)
    .await?;

    // Calculate improvement % vs worst time slot
    let improvement_pct = if let Some(ref bt) = best_time {
        let worst_rate: Option<f64> = sqlx::query_scalar(
            r#"
            SELECT MAX(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)))
            FROM sessions
            WHERE first_message_at IS NOT NULL
                AND files_edited_count > 0
                AND last_message_at >= ?1 AND last_message_at <= ?2
            GROUP BY
                CASE CAST(strftime('%w', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER)
                    WHEN 0 THEN 'Sunday' WHEN 1 THEN 'Monday' WHEN 2 THEN 'Tuesday'
                    WHEN 3 THEN 'Wednesday' WHEN 4 THEN 'Thursday' WHEN 5 THEN 'Friday'
                    ELSE 'Saturday'
                END,
                CASE
                    WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 6 AND 11 THEN 'morning'
                    WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 12 AND 17 THEN 'afternoon'
                    WHEN CAST(strftime('%H', datetime(first_message_at, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 18 AND 22 THEN 'evening'
                    ELSE 'night'
                END
            "#
        )
        .bind(from_ts)
        .bind(to_ts)
        .fetch_one(pool)
        .await?;

        if let Some(worst) = worst_rate {
            if worst > 0.0 {
                ((worst - bt.avg_reedit_rate) / worst * 100.0).max(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    Ok(InsightsOverview {
        work_breakdown: WorkBreakdown {
            total_sessions: work.total_sessions as u32,
            with_commits: work.with_commits as u32,
            exploration: work.exploration as u32,
            avg_session_minutes: work.avg_duration_seconds / 60.0,
        },
        efficiency: EfficiencyStats {
            avg_reedit_rate: eff.avg_reedit_rate.unwrap_or(0.0),
            avg_edit_velocity: eff.avg_edit_velocity.unwrap_or(0.0),
            trend,
            trend_pct,
        },
        best_time: BestTimeStats {
            day_of_week: best_time.as_ref().map(|b| b.day_of_week.clone()).unwrap_or_default(),
            time_slot: best_time.as_ref().map(|b| b.time_slot.clone()).unwrap_or_default(),
            improvement_pct,
        },
    })
}

/// Calculate efficiency trend by comparing recent vs earlier periods
async fn calculate_efficiency_trend(
    pool: &SqlitePool,
    from_ts: i64,
    mid_ts: i64,
    to_ts: i64,
) -> Result<(String, f64), PatternError> {
    #[derive(sqlx::FromRow)]
    struct PeriodStats {
        avg_reedit_rate: Option<f64>,
    }

    let earlier: PeriodStats = sqlx::query_as(
        r#"
        SELECT AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
        FROM sessions
        WHERE files_edited_count > 0 AND last_message_at >= ?1 AND last_message_at < ?2
        "#
    )
    .bind(from_ts)
    .bind(mid_ts)
    .fetch_one(pool)
    .await?;

    let recent: PeriodStats = sqlx::query_as(
        r#"
        SELECT AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
        FROM sessions
        WHERE files_edited_count > 0 AND last_message_at >= ?1 AND last_message_at <= ?2
        "#
    )
    .bind(mid_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    let earlier_rate = earlier.avg_reedit_rate.unwrap_or(0.0);
    let recent_rate = recent.avg_reedit_rate.unwrap_or(0.0);

    if earlier_rate == 0.0 {
        return Ok(("stable".to_string(), 0.0));
    }

    // Lower reedit rate = better, so improvement is when recent < earlier
    let change_pct = ((earlier_rate - recent_rate) / earlier_rate) * 100.0;

    let trend = if change_pct > 5.0 {
        "improving"
    } else if change_pct < -5.0 {
        "declining"
    } else {
        "stable"
    };

    Ok((trend.to_string(), change_pct.abs()))
}

/// Get classification status for sessions
async fn get_classification_status(pool: &SqlitePool) -> Result<ClassificationStatus, PatternError> {
    #[derive(sqlx::FromRow)]
    struct ClassStats {
        total: i64,
        classified: i64,
    }

    let stats: ClassStats = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(CASE WHEN category_l1 IS NOT NULL THEN 1 END) as classified
        FROM sessions
        "#
    )
    .fetch_one(pool)
    .await?;

    let pending = (stats.total - stats.classified) as u32;
    let classification_pct = if stats.total > 0 {
        (stats.classified as f64 / stats.total as f64) * 100.0
    } else {
        0.0
    };

    Ok(ClassificationStatus {
        classified: stats.classified as u32,
        total: stats.total as u32,
        pending_classification: pending,
        classification_pct,
    })
}
```

---

## Rust Implementation

### Module Structure

```
crates/core/src/
├── patterns/
│   ├── mod.rs           # Pattern trait + registry
│   ├── prompt.rs        # P01-P10 patterns
│   ├── session.rs       # S01-S08 patterns
│   ├── temporal.rs      # T01-T07 patterns
│   ├── workflow.rs      # W01-W08 patterns
│   ├── model.rs         # M01-M05 patterns
│   ├── codebase.rs      # C01-C07 patterns
│   ├── outcome.rs       # O01-O05 patterns
│   ├── behavioral.rs    # B01-B07 patterns
│   └── comparative.rs   # CP01-CP03 patterns
├── insights/
│   ├── mod.rs           # Insight generation
│   ├── scoring.rs       # Impact scoring algorithm
│   ├── templates.rs     # Text templates
│   └── generator.rs     # Insight text generator
└── lib.rs               # Re-exports

crates/server/src/routes/
└── insights.rs          # GET /api/insights handler
```

### Pattern Trait

```rust
// crates/core/src/patterns/mod.rs

use async_trait::async_trait;
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PatternResult {
    pub pattern_id: String,
    pub category: String,
    pub data: HashMap<String, serde_json::Value>,
    pub sample_size: u32,
    pub actionability: Actionability,
}

#[async_trait]
pub trait Pattern: Send + Sync {
    /// Unique pattern identifier (e.g., "P01")
    fn id(&self) -> &'static str;

    /// Human-readable category (e.g., "Prompt Patterns")
    fn category(&self) -> &'static str;

    /// Minimum sample size for this pattern
    fn min_sample_size(&self) -> u32;

    /// How actionable is this insight
    fn actionability(&self) -> Actionability;

    /// Calculate the pattern from database
    async fn calculate(
        &self,
        pool: &SqlitePool,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Option<PatternResult>, PatternError>;

    /// Generate insight text from pattern data
    fn generate_insight(&self, result: &PatternResult) -> Option<GeneratedInsight>;
}

#[derive(Clone, Copy, Debug)]
pub enum Actionability {
    Immediate,
    Moderate,
    Awareness,
    Informational,
}

/// Pattern calculation errors
#[derive(Debug, thiserror::Error)]
pub enum PatternError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Insufficient data: need {required} samples, got {actual}")]
    InsufficientData { required: u32, actual: u32 },

    #[error("Invalid calculation: {0}")]
    InvalidCalculation(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

// Pattern registry
pub struct PatternRegistry {
    patterns: Vec<Box<dyn Pattern>>,
    pattern_index: std::collections::HashMap<String, usize>,
}

impl PatternRegistry {
    pub fn new() -> Self {
        let mut patterns: Vec<Box<dyn Pattern>> = Vec::new();

        // Register all patterns
        patterns.push(Box::new(prompt::PromptLengthPattern));
        patterns.push(Box::new(prompt::QuestionVsCommandPattern));
        // ... register all 60+ patterns

        // Build index for O(1) lookup
        let pattern_index = patterns
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id().to_string(), i))
            .collect();

        Self { patterns, pattern_index }
    }

    /// Get a pattern by its ID (e.g., "P01", "S01")
    pub fn get_pattern(&self, pattern_id: &str) -> Option<&dyn Pattern> {
        self.pattern_index
            .get(pattern_id)
            .map(|&idx| self.patterns[idx].as_ref())
    }

    /// Calculate all patterns with semaphore-bounded parallelism
    /// Per CLAUDE.md: CPU-bound work must use semaphore bounded to num_cpus
    pub async fn calculate_all(
        &self,
        pool: &SqlitePool,
        from_ts: i64,
        to_ts: i64,
    ) -> Vec<PatternResult> {
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        let parallelism = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        let semaphore = Arc::new(Semaphore::new(parallelism));

        let mut handles = Vec::with_capacity(self.patterns.len());

        for pattern in &self.patterns {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let pool = pool.clone();
            let pattern_id = pattern.id().to_string();

            // Clone pattern data needed for async block
            // Note: Pattern trait objects can't be sent across threads easily,
            // so we calculate inline but bound concurrency with semaphore
            handles.push(tokio::spawn(async move {
                let _permit = permit; // Hold permit for duration
                // Pattern calculation happens here
                // In real impl, pass pattern reference or use Arc<dyn Pattern>
                drop(_permit);
                (pattern_id, None::<PatternResult>) // Placeholder
            }));
        }

        // Actual implementation: calculate sequentially but yield between patterns
        // This is simpler and still allows other tasks to run
        let mut results = Vec::new();
        for pattern in &self.patterns {
            match pattern.calculate(pool, from_ts, to_ts).await {
                Ok(Some(result)) => results.push(result),
                Ok(None) => {} // Insufficient data
                Err(e) => tracing::warn!("Pattern {} failed: {}", pattern.id(), e),
            }
            // Yield to allow other tasks to run
            tokio::task::yield_now().await;
        }

        results
    }
}
```

### Example Pattern Implementation

```rust
// crates/core/src/patterns/session.rs

use super::*;

pub struct OptimalDurationPattern;

#[async_trait]
impl Pattern for OptimalDurationPattern {
    fn id(&self) -> &'static str { "S01" }
    fn category(&self) -> &'static str { "Session Patterns" }
    fn min_sample_size(&self) -> u32 { 50 }
    fn actionability(&self) -> Actionability { Actionability::Moderate }

    async fn calculate(
        &self,
        pool: &SqlitePool,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Option<PatternResult>, PatternError> {
        #[derive(sqlx::FromRow)]
        struct DurationBucket {
            duration_bucket: String,
            session_count: i64,
            edits_per_minute: f64,
            avg_reedit_rate: f64,
        }

        let rows: Vec<DurationBucket> = sqlx::query_as(
            r#"
            SELECT
              CASE
                WHEN duration_seconds < 900 THEN '<15min'
                WHEN duration_seconds < 2700 THEN '15-45min'
                WHEN duration_seconds < 5400 THEN '45-90min'
                ELSE '>90min'
              END as duration_bucket,
              COUNT(*) as session_count,
              AVG(CAST(files_edited_count AS REAL) / (duration_seconds / 60.0)) as edits_per_minute,
              AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate
            FROM sessions
            WHERE duration_seconds > 0 AND files_edited_count > 0
              AND last_message_at >= ?1 AND last_message_at <= ?2
            GROUP BY duration_bucket
            HAVING COUNT(*) >= 10
            "#
        )
        .bind(from_ts)
        .bind(to_ts)
        .fetch_all(pool)
        .await?;

        let total_sessions: u32 = rows.iter().map(|r| r.session_count as u32).sum();

        if total_sessions < self.min_sample_size() {
            return Ok(None);
        }

        // Find best and worst buckets
        let best = rows.iter()
            .max_by(|a, b| a.edits_per_minute.partial_cmp(&b.edits_per_minute).unwrap())
            .unwrap();
        let worst = rows.iter()
            .min_by(|a, b| a.edits_per_minute.partial_cmp(&b.edits_per_minute).unwrap())
            .unwrap();

        let improvement = if worst.edits_per_minute > 0.0 {
            ((best.edits_per_minute - worst.edits_per_minute) / worst.edits_per_minute) * 100.0
        } else {
            0.0
        };

        let mut data = HashMap::new();
        data.insert("optimal_duration".to_string(), json!(best.duration_bucket));
        data.insert("worst_duration".to_string(), json!(worst.duration_bucket));
        data.insert("best_velocity".to_string(), json!(best.edits_per_minute));
        data.insert("worst_velocity".to_string(), json!(worst.edits_per_minute));
        data.insert("improvement_pct".to_string(), json!(improvement));
        data.insert("buckets".to_string(), json!(rows.iter().map(|r| {
            json!({
                "bucket": r.duration_bucket,
                "count": r.session_count,
                "velocity": r.edits_per_minute,
                "reedit_rate": r.avg_reedit_rate,
            })
        }).collect::<Vec<_>>()));

        Ok(Some(PatternResult {
            pattern_id: self.id().to_string(),
            category: self.category().to_string(),
            data,
            sample_size: total_sessions,
            actionability: self.actionability(),
        }))
    }

    fn generate_insight(&self, result: &PatternResult) -> Option<GeneratedInsight> {
        let optimal = result.data.get("optimal_duration")?.as_str()?;
        let worst = result.data.get("worst_duration")?.as_str()?;
        let improvement = result.data.get("improvement_pct")?.as_f64()?;

        let score = calculate_pattern_score(
            improvement / 100.0,  // effect size
            result.sample_size,
            self.min_sample_size(),
            result.actionability,
        );

        Some(GeneratedInsight {
            pattern_id: self.id().to_string(),
            category: self.category().to_string(),
            title: "Session Duration Sweet Spot".to_string(),
            body: format!(
                "Your {} sessions produce {:.0}% more edits per minute than {} sessions.",
                optimal, improvement, worst
            ),
            recommendation: Some(format!(
                "Consider breaking longer sessions into {} chunks.",
                optimal
            )),
            impact_score: score.combined,
            impact_tier: score.tier().to_string(),
            evidence: InsightEvidence {
                sample_size: result.sample_size,
                time_range_days: 30, // from API params
                comparison_values: result.data.iter()
                    .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                    .collect(),
            },
        })
    }
}
```

### Insights Route Handler

```rust
// crates/server/src/routes/insights.rs

use axum::{extract::{Query, State}, Json};
use serde::Deserialize;
use claude_view_core::patterns::PatternRegistry;

#[derive(Debug, Deserialize)]
pub struct InsightsQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub min_impact: Option<f64>,
    pub categories: Option<String>,
    pub limit: Option<u32>,
}

pub async fn get_insights(
    State(state): State<AppState>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<InsightsResponse>, ApiError> {
    let now = chrono::Utc::now().timestamp();
    let from = query.from.unwrap_or(now - 30 * 86400);
    let to = query.to.unwrap_or(now);
    let min_impact = query.min_impact.unwrap_or(0.3);
    let limit = query.limit.unwrap_or(50);

    // Check cache first
    let cache_key = format!("insights:{}:{}:{}", from, to, min_impact);
    if let Some(cached) = state.insight_cache.get(&cache_key).await {
        return Ok(Json(cached));
    }

    // Calculate all patterns
    let registry = PatternRegistry::new();
    let pattern_results = registry.calculate_all(&state.db.pool(), from, to).await;

    // Generate insights and score them
    let mut insights: Vec<GeneratedInsight> = pattern_results
        .iter()
        .filter_map(|result| {
            let pattern = registry.get_pattern(&result.pattern_id)?;
            pattern.generate_insight(result)
        })
        .filter(|insight| insight.impact_score >= min_impact)
        .collect();

    // Sort by impact score descending
    insights.sort_by(|a, b| b.impact_score.partial_cmp(&a.impact_score).unwrap());

    // Group by tier
    let high: Vec<_> = insights.iter()
        .filter(|i| i.impact_tier == "high")
        .take(limit as usize / 3)
        .cloned()
        .collect();
    let medium: Vec<_> = insights.iter()
        .filter(|i| i.impact_tier == "medium")
        .take(limit as usize / 3)
        .cloned()
        .collect();
    let observations: Vec<_> = insights.iter()
        .filter(|i| i.impact_tier == "observation")
        .take(limit as usize / 3)
        .cloned()
        .collect();

    // Build overview stats
    let overview = calculate_overview(&state.db.pool(), from, to).await?;

    // Classification status
    let classification_status = get_classification_status(&state.db.pool()).await?;

    let response = InsightsResponse {
        top_insight: high.first().cloned(),
        overview,
        patterns: PatternGroups { high, medium, observations },
        classification_status,
        meta: InsightsMeta {
            computed_at: now,
            time_range_start: from,
            time_range_end: to,
            patterns_evaluated: pattern_results.len() as u32,
            patterns_returned: insights.len() as u32,
        },
    };

    // Cache for 5 minutes
    state.insight_cache.set(&cache_key, &response, 300).await;

    Ok(Json(response))
}
```

---

## Caching Strategy

### Cache Invalidation Rules

| Event | Cache Action |
|-------|--------------|
| New session indexed | Invalidate all insights caches |
| Session updated (re-edit) | Invalidate affected time range |
| Classification completed | Invalidate category-related caches |
| Time passes (5 min) | Automatic TTL expiration |

### Cache Implementation

```rust
use moka::future::Cache;
use std::sync::Arc;

pub struct InsightCache {
    cache: Cache<String, InsightsResponse>,
}

impl InsightCache {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(300))
                .max_capacity(100)
                .build(),
        }
    }

    pub async fn get(&self, key: &str) -> Option<InsightsResponse> {
        self.cache.get(key).await
    }

    pub async fn set(&self, key: &str, value: &InsightsResponse, _ttl_secs: u64) {
        self.cache.insert(key.to_string(), value.clone()).await;
    }

    pub async fn invalidate_all(&self) {
        self.cache.invalidate_all();
    }

    pub async fn invalidate_time_range(&self, from: i64, to: i64) {
        // Invalidate any cache keys that overlap with the given range
        // This is a simplified approach; production might use more granular invalidation
        self.cache.invalidate_all();
    }
}
```

### Recalculation Triggers

1. **On-demand**: API request misses cache
2. **Background refresh**: Every 15 minutes for active users
3. **Event-driven**: Session indexing completes

---

## Testing Strategy

### Unit Tests

```rust
// crates/core/src/patterns/session_tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        // Run migrations
        sqlx::query(include_str!("../../db/migrations.sql"))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_optimal_duration_pattern_insufficient_data() {
        let pool = setup_test_db().await;
        let pattern = OptimalDurationPattern;

        let result = pattern.calculate(&pool, 0, i64::MAX).await.unwrap();
        assert!(result.is_none(), "Should return None for insufficient data");
    }

    #[tokio::test]
    async fn test_optimal_duration_pattern_with_data() {
        let pool = setup_test_db().await;

        // Insert 100 test sessions with varying durations
        for i in 0..100 {
            let duration = match i % 4 {
                0 => 600,   // 10 min
                1 => 1800,  // 30 min
                2 => 3600,  // 60 min
                _ => 5400,  // 90 min
            };
            let files_edited = if duration == 1800 { 10 } else { 5 };
            let reedited = if duration == 1800 { 1 } else { 2 };

            sqlx::query(r#"
                INSERT INTO sessions (id, project_id, file_path, preview,
                    duration_seconds, files_edited_count, reedited_files_count,
                    last_message_at)
                VALUES (?1, 'proj', '/tmp/t.jsonl', 'test', ?2, ?3, ?4, ?5)
            "#)
            .bind(format!("sess-{}", i))
            .bind(duration)
            .bind(files_edited)
            .bind(reedited)
            .bind(1000000 + i)
            .execute(&pool)
            .await
            .unwrap();
        }

        let pattern = OptimalDurationPattern;
        let result = pattern.calculate(&pool, 0, i64::MAX).await.unwrap();

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.pattern_id, "S01");
        assert!(result.sample_size >= 50);

        let insight = pattern.generate_insight(&result);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert!(insight.body.contains("15-45min"));
    }

    #[tokio::test]
    async fn test_impact_scoring() {
        // Large effect, good sample, immediate actionability
        let score = PatternScore::calculate(0.8, 0.9, 1.0);
        assert!(score.combined > 0.8);
        assert_eq!(score.tier(), "high");

        // Small effect, small sample, low actionability
        let score = PatternScore::calculate(0.1, 0.3, 0.2);
        assert!(score.combined < 0.3);
        assert_eq!(score.tier(), "observation");
    }

    #[tokio::test]
    async fn test_template_rendering() {
        let mut vars = HashMap::new();
        vars.insert("optimal_duration".to_string(), "15-45min".to_string());
        vars.insert("improvement".to_string(), "35.5".to_string());
        vars.insert("worst_duration".to_string(), ">90min".to_string());

        let template = "Your {optimal_duration} sessions produce {improvement:.0}% more edits per minute than {worst_duration} sessions.";
        let rendered = render_template(template, &vars);

        assert_eq!(
            rendered,
            "Your 15-45min sessions produce 36% more edits per minute than >90min sessions."
        );
    }
}
```

### Integration Tests

```rust
// crates/server/tests/insights_api_test.rs

#[tokio::test]
async fn test_insights_endpoint_empty_db() {
    let app = create_test_app().await;

    let response = app
        .oneshot(Request::builder()
            .uri("/api/insights")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: InsightsResponse = parse_body(response).await;
    assert!(body.top_insight.is_none());
    assert!(body.patterns.high.is_empty());
}

#[tokio::test]
async fn test_insights_endpoint_with_data() {
    let app = create_test_app().await;

    // Seed with 100 sessions
    seed_test_sessions(&app.db, 100).await;

    let response = app
        .oneshot(Request::builder()
            .uri("/api/insights?min_impact=0.3")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: InsightsResponse = parse_body(response).await;
    assert!(body.meta.patterns_evaluated > 0);
    // With sufficient data, should have at least some insights
    assert!(!body.patterns.high.is_empty() || !body.patterns.medium.is_empty());
}

#[tokio::test]
async fn test_insights_time_range_filter() {
    let app = create_test_app().await;

    // Seed sessions at different times
    let now = chrono::Utc::now().timestamp();
    seed_sessions_at_time(&app.db, 50, now - 7 * 86400).await;  // Last week
    seed_sessions_at_time(&app.db, 50, now - 60 * 86400).await; // 2 months ago

    // Query only last 30 days
    let from = now - 30 * 86400;
    let response = app
        .oneshot(Request::builder()
            .uri(format!("/api/insights?from={}", from))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    let body: InsightsResponse = parse_body(response).await;

    // Should only include recent sessions
    assert!(body.overview.work_breakdown.total_sessions <= 50);
}
```

### Golden File Tests

Store expected outputs for regression testing:

```rust
#[tokio::test]
async fn test_insight_generation_golden() {
    let pattern_result = PatternResult {
        pattern_id: "S01".to_string(),
        category: "Session Patterns".to_string(),
        data: serde_json::from_str(include_str!("golden/s01_input.json")).unwrap(),
        sample_size: 150,
        actionability: Actionability::Moderate,
    };

    let pattern = OptimalDurationPattern;
    let insight = pattern.generate_insight(&pattern_result).unwrap();

    let expected: GeneratedInsight =
        serde_json::from_str(include_str!("golden/s01_output.json")).unwrap();

    assert_eq!(insight.title, expected.title);
    assert_eq!(insight.body, expected.body);
    assert!((insight.impact_score - expected.impact_score).abs() < 0.01);
}
```

---

## Acceptance Criteria

### AC-4.1: Pattern Calculation Functions

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.1.1 | Calculate P01-P10 patterns | Returns valid PatternResult | |
| 4.1.2 | Calculate S01-S08 patterns | Returns valid PatternResult | |
| 4.1.3 | Calculate T01-T07 patterns | Returns valid PatternResult | |
| 4.1.4 | Calculate W01-W08 patterns | Returns valid PatternResult | |
| 4.1.5 | Calculate M01-M05 patterns | Returns valid PatternResult | |
| 4.1.6 | Calculate C01-C07 patterns | Returns valid PatternResult | |
| 4.1.7 | Calculate O01-O05 patterns | Returns valid PatternResult | |
| 4.1.8 | Calculate B01-B07 patterns | Returns valid PatternResult | |
| 4.1.9 | Calculate CP01-CP03 patterns | Returns valid PatternResult | |
| 4.1.10 | Insufficient data | Returns None gracefully | |
| 4.1.11 | Empty database | Returns None, no errors | |
| 4.1.12 | Division by zero handling | Returns None, no panic | |

### AC-4.2: Impact Scoring Algorithm

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.2.1 | Large effect + high sample + actionable | Score > 0.7, tier = "high" | |
| 4.2.2 | Medium effect + medium sample | Score 0.4-0.7, tier = "medium" | |
| 4.2.3 | Small effect or low sample | Score < 0.4, tier = "observation" | |
| 4.2.4 | Score normalization | All scores 0.0-1.0 | |
| 4.2.5 | Cross-pattern comparison | Fair ranking across categories | |

### AC-4.3: Insight Text Generation

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.3.1 | Template variable substitution | All variables replaced | |
| 4.3.2 | Numeric formatting | Proper decimal places | |
| 4.3.3 | Missing variable | Graceful fallback | |
| 4.3.4 | Recommendation generation | Actionable text when available | |
| 4.3.5 | Natural language quality | Reads as human-written | |

### AC-4.4: GET /api/insights Endpoint

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.4.1 | Default request | Returns top insight + patterns | |
| 4.4.2 | Time range filter | Only includes matching sessions | |
| 4.4.3 | min_impact filter | Excludes low-scoring patterns | |
| 4.4.4 | categories filter | Only matching categories | |
| 4.4.5 | limit parameter | Respects max patterns | |
| 4.4.6 | Cache hit | Returns cached response < 10ms | |
| 4.4.7 | Cache miss | Calculates and caches | |
| 4.4.8 | Response structure | Matches TypeScript interface | |
| 4.4.9 | Performance | < 500ms for 10k sessions | |

### AC-4.5: Error Handling

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.5.1 | Database error | 500 with error message | |
| 4.5.2 | Invalid time range | 400 Bad Request | |
| 4.5.3 | Partial pattern failure | Returns successful patterns | |

---

## Performance Requirements

| Metric | Target | Measurement |
|--------|--------|-------------|
| Pattern calculation (single) | < 50ms | p95 latency |
| All patterns calculation | < 2s | Total time, 10k sessions |
| Insight generation | < 10ms per pattern | CPU time |
| API response (cache miss) | < 500ms | E2E latency |
| API response (cache hit) | < 10ms | E2E latency |
| Memory usage | < 100MB | Peak during calculation |

### Optimization Strategies

1. **Parallel pattern calculation**: Use `tokio::join!` for independent patterns
2. **Batch SQL queries**: Combine related patterns into single queries
3. **Incremental updates**: Only recalculate affected patterns on new data
4. **Index coverage**: Ensure all pattern queries use indexes

---

## Dependencies

### Required from Phase 1

- Database schema with session metrics (files_edited_count, reedited_files_count, etc.)
- Classification columns (category_l1, category_l2, category_l3) - optional, graceful degradation
- index_metadata table for tracking

### External Crates

```toml
# crates/core/Cargo.toml additions
[dependencies]
async-trait = "0.1"
moka = { version = "0.12", features = ["future"] }
```

---

## Future Enhancements

1. **Custom patterns**: User-defined pattern rules
2. **Pattern explanations**: LLM-generated insight elaborations
3. **Comparative benchmarks**: Anonymous aggregate comparisons
4. **Pattern alerts**: Notify when significant patterns emerge
5. **ML-based patterns**: Unsupervised pattern discovery

---

## Related Documents

- Master design: `../2026-02-05-theme4-chat-insights-design.md`
- Phase 1 (Foundation): `phase1-foundation.md`
- Phase 5 (Insights Core): `phase5-insights-core.md`
- Progress tracker: `PROGRESS.md`
