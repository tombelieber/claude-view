---
status: draft
date: 2026-01-27
---

# claude-view Analytics - Design Specification

> Productivity analytics for vibe coders, built into claude-view Phase 2

**Status:** Draft — partially implemented via Phase 3 Metrics Engine, CLI stats + insights generation still pending
**Date:** 2026-01-27

---

## 1. Problem Statement

Vibe coders run many AI coding sessions but lack insight into:
- Which workflows lead to smooth, productive sessions
- Which skills/patterns correlate with shipping code faster
- What sessions were turbulent (high back-and-forth, lots of rework)
- Patterns that should become skills, sub-agents, or MCPs

---

## 2. Goals

| Goal | Metric |
|------|--------|
| Identify smooth vs turbulent sessions | Session health classification |
| Correlate sessions with git commits | Auto-link shipped code |
| Surface actionable patterns | Skills effectiveness, rework signals |
| Zero manual work | All metrics auto-detected |

---

## 3. Scope

### In Scope (MVP)
- Session health metrics (turn count, circle-back rate, duration)
- Git commit correlation (auto-detected via file overlap + time)
- Dashboard with stats banner + enriched session list
- CLI report (`claude-view stats`)
- Smooth/turbulent classification

### Out of Scope (Post-MVP)
- Email digest (defer until traction)
- Multi-AI support (Claude-first, Codex/Gemini/Cursor later)
- Manual session rating (start with flow metrics only)
- LLM-powered insight generation

---

## 4. Architecture

Analytics is **Phase 2 of claude-view**, same codebase.

```
┌─────────────────────────────────────────────────────────┐
│                    claude-view                          │
├─────────────────────────────────────────────────────────┤
│  Phase 1: Search + Tagging (current)                    │
│  Phase 2: Analytics (this design)                       │
└─────────────────────────────────────────────────────────┘
```

**New crate:** `crates/analytics/`

```
crates/analytics/
├── src/
│   ├── lib.rs
│   ├── metrics.rs      # Turn count, duration, circle-back calculation
│   ├── git.rs          # Git commit discovery and correlation
│   ├── health.rs       # Smooth/turbulent classification
│   └── insights.rs     # Pattern detection
```

---

## 5. Core Metrics

| Metric | Definition | Signal |
|--------|------------|--------|
| **Turn count** | Number of user↔assistant exchanges | Session friction |
| **Circle-back rate** | % of turns that revisit previous topic/file | Rework signal |
| **Session duration** | First message → last message | Time investment |
| **Time to commit** | Session end → linked commit timestamp | Shipping velocity |
| **Skill usage** | Skills invoked during session | Workflow patterns |

### 5.1 Circle-back Detection

A "circle-back" is detected when:
1. Same file is edited again after 3+ turns
2. Same error/topic mentioned after being "resolved"
3. User says "wait", "actually", "go back", "that's wrong"

```rust
pub fn detect_circle_backs(session: &Session) -> Vec<CircleBack> {
    let mut file_last_seen: HashMap<PathBuf, usize> = HashMap::new();
    let mut circle_backs = vec![];

    for (turn_num, turn) in session.turns.iter().enumerate() {
        for file in &turn.files_touched {
            if let Some(last_turn) = file_last_seen.get(file) {
                if turn_num - last_turn >= 3 {
                    circle_backs.push(CircleBack {
                        file: file.clone(),
                        first_turn: *last_turn,
                        return_turn: turn_num,
                    });
                }
            }
            file_last_seen.insert(file.clone(), turn_num);
        }
    }
    circle_backs
}
```

---

## 6. Git Commit Correlation

### 6.1 Approach

**Direction:** Commit → find sessions (not session → find commits)

This handles the "commit late" workflow where users stage changes for days.

### 6.2 Algorithm

```rust
pub fn correlate_commit(commit: &GitCommit, sessions: &[Session]) -> Vec<SessionLink> {
    let commit_files: HashSet<_> = commit.files.iter().collect();

    sessions
        .iter()
        .filter_map(|session| {
            let session_files: HashSet<_> = session.files_edited().collect();
            let overlap: HashSet<_> = commit_files.intersection(&session_files).collect();

            if overlap.is_empty() {
                return None;
            }

            let file_score = overlap.len() as f64 / commit_files.len() as f64;
            let days_ago = (commit.timestamp - session.ended_at).num_days() as f64;
            let recency_score = 0.5_f64.powf(days_ago / 3.0); // half-life = 3 days

            let confidence = file_score * 0.7 + recency_score * 0.3;

            Some(SessionLink {
                session_id: session.id.clone(),
                commit_hash: commit.hash.clone(),
                confidence,
                overlapping_files: overlap.into_iter().cloned().collect(),
            })
        })
        .filter(|link| link.confidence >= 0.40)
        .collect()
}
```

### 6.3 Confidence Thresholds

| Confidence | Display | Counts in Stats |
|------------|---------|-----------------|
| ≥ 0.60 | "Shipped" (no qualifier) | Yes |
| 0.40 - 0.59 | "Possibly shipped" | No |
| < 0.40 | Not shown | No |

### 6.4 User Corrections

One-click corrections (optional, not required):
- **[This is correct]** → Boosts confidence for similar future matches
- **[Unlink]** → Removes this specific link

Corrections stored in SQLite, used to tune matching over time.

---

## 7. Session Health Classification

### 7.1 Smooth vs Turbulent

```rust
pub enum SessionHealth {
    Smooth,
    Turbulent,
    Neutral,
}

pub fn classify_health(session: &Session) -> SessionHealth {
    let dominated_by_turbulence =
        session.turn_count > 15 ||
        session.circle_back_rate() > 0.25 ||
        (session.duration_minutes() > 30 && session.commits_linked == 0);

    let clearly_smooth =
        session.turn_count < 8 &&
        session.duration_minutes() < 20 &&
        session.commits_linked >= 1 &&
        session.circle_back_rate() < 0.10;

    if dominated_by_turbulence {
        SessionHealth::Turbulent
    } else if clearly_smooth {
        SessionHealth::Smooth
    } else {
        SessionHealth::Neutral
    }
}
```

### 7.2 Thresholds

| Condition | Smooth | Neutral | Turbulent |
|-----------|--------|---------|-----------|
| Turn count | < 8 | 8-15 | > 15 |
| Duration | < 20min | 20-30min | > 30min (no commit) |
| Circle-back rate | < 10% | 10-25% | > 25% |
| Commits linked | ≥ 1 | 0 (short session OK) | 0 (long session) |

---

## 8. Database Schema Additions

```sql
-- Session metrics (extends existing sessions table)
ALTER TABLE sessions ADD COLUMN turn_count INTEGER DEFAULT 0;
ALTER TABLE sessions ADD COLUMN duration_seconds INTEGER;
ALTER TABLE sessions ADD COLUMN circle_back_count INTEGER DEFAULT 0;
ALTER TABLE sessions ADD COLUMN health TEXT CHECK(health IN ('smooth', 'neutral', 'turbulent'));

-- Git commit links
CREATE TABLE session_commits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    commit_hash TEXT NOT NULL,
    confidence REAL NOT NULL,
    user_verified INTEGER DEFAULT 0,  -- 1 = user confirmed, -1 = user unlinked
    linked_at INTEGER DEFAULT (unixepoch()),
    UNIQUE(session_id, commit_hash)
);

CREATE INDEX idx_session_commits_session ON session_commits(session_id);
CREATE INDEX idx_session_commits_hash ON session_commits(commit_hash);

-- Commit metadata cache
CREATE TABLE commits (
    hash TEXT PRIMARY KEY,
    repo_path TEXT NOT NULL,
    message TEXT,
    author TEXT,
    timestamp INTEGER NOT NULL,
    files_changed TEXT,  -- JSON array
    insertions INTEGER,
    deletions INTEGER,
    indexed_at INTEGER DEFAULT (unixepoch())
);

-- Daily aggregates for fast dashboard queries
CREATE TABLE daily_stats (
    date TEXT PRIMARY KEY,  -- YYYY-MM-DD
    session_count INTEGER DEFAULT 0,
    smooth_count INTEGER DEFAULT 0,
    turbulent_count INTEGER DEFAULT 0,
    total_turns INTEGER DEFAULT 0,
    total_commits INTEGER DEFAULT 0,
    total_duration_seconds INTEGER DEFAULT 0
);
```

---

## 9. API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/stats` | GET | Dashboard summary (week/month/all-time) |
| `/api/stats/daily` | GET | Daily breakdown for charts |
| `/api/sessions/:id/commits` | GET | Commits linked to session |
| `/api/sessions/:id/commits/:hash` | POST | Verify link (user correction) |
| `/api/sessions/:id/commits/:hash` | DELETE | Unlink (user correction) |
| `/api/skills/effectiveness` | GET | Skills ranked by smooth rate |

### 9.1 Stats Response

```json
{
  "period": "week",
  "start_date": "2026-01-20",
  "end_date": "2026-01-27",
  "sessions": {
    "total": 23,
    "smooth": 15,
    "turbulent": 4,
    "neutral": 4,
    "change_vs_previous": 5
  },
  "metrics": {
    "avg_turns": 4.2,
    "avg_duration_minutes": 12,
    "circle_back_rate": 0.08,
    "commits_linked": 18
  },
  "trends": {
    "avg_turns_change": -0.8,
    "avg_duration_change": -3,
    "circle_back_rate_change": -0.02
  },
  "top_skills": [
    {"name": "/commit", "uses": 18, "smooth_rate": 0.92},
    {"name": "/brainstorm", "uses": 7, "smooth_rate": 0.85},
    {"name": "/debug", "uses": 4, "smooth_rate": 0.25}
  ],
  "insights": [
    "Your smoothest sessions start with /brainstorm",
    "/debug sessions average 14 turns — consider breaking problems smaller",
    "You shipped 3x more on Tue/Wed than other days"
  ]
}
```

---

## 10. UI Design

### 10.1 Dashboard Layout

```
┌─────────────────────────────────────────────────────────────┐
│  claude-view                                    [Search] 🔍 │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  📊 This Week                              [▼ Collapse]     │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │   23    │ │   4.2   │ │  12min  │ │   8%    │           │
│  │sessions │ │avg turns│ │ to ship │ │rework   │           │
│  │ +5 ↑    │ │ -0.8 ↓  │ │ -3min ↓ │ │ -2% ↓   │           │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
│                                                             │
│  Top skill: /commit (92% smooth rate)                       │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  Sessions                          [Filter ▼] [Skills ▼]    │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────┐  │
│  │ claude-view v2 design          Today 2:15am           │  │
│  │ 🔄 6 turns  ⏱ 18min  ✨ smooth                        │  │
│  │ /brainstorm → /commit                                 │  │
│  │                                                       │  │
│  │ 📦 Shipped                                            │  │
│  │    feat: add analytics dashboard  ──────  +142 -23    │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Fix auth bug                   Yesterday 11pm         │  │
│  │ 🔄 14 turns  ⏱ 42min  ⚠️ turbulent                    │  │
│  │ /debug → /debug → /debug                              │  │
│  │                                                       │  │
│  │ 📦 Possibly shipped                    [See why ▾]    │  │
│  │    fix: auth middleware       ──────   +23 -8         │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Explore codebase              3 days ago              │  │
│  │ 🔄 4 turns  ⏱ 5min                                    │  │
│  │                                                       │  │
│  │ 📭 No commits linked                                  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 10.2 "Possibly Shipped" Dropdown

```
📦 Possibly shipped                            [See why ▾]
   ┌─────────────────────────────────────────────────────┐
   │  Why "possibly"?                                    │
   │                                                     │
   │  ✓ Files match: src/auth/middleware.ts              │
   │  ⚠ Committed 3 days after session                   │
   │                                                     │
   │  [This is correct]  [Unlink]                        │
   └─────────────────────────────────────────────────────┘
```

### 10.3 Visual Indicators

| Health | Icon | Color |
|--------|------|-------|
| Smooth | ✨ | Green accent (subtle) |
| Turbulent | ⚠️ | Orange accent (subtle) |
| Neutral | (none) | Default |

---

## 11. CLI Design

### 11.1 Command

```bash
claude-view stats [--day|--week|--month] [--json] [--no-color]
```

### 11.2 Output

```
$ claude-view stats

╭──────────────────────────────────────────────────────────────╮
│  📈 Your Vibe Coding · Jan 20-27                             │
╰──────────────────────────────────────────────────────────────╯

  SESSIONS        23        ████████████░░░░  +5 vs last week
  AVG TURNS       4.2       ██████░░░░░░░░░░  ↓ better (was 5.0)
  TIME TO SHIP    12 min    ████████░░░░░░░░  ↓ faster (was 15)
  REWORK RATE     8%        ██░░░░░░░░░░░░░░  ↓ cleaner (was 10%)

╭──────────────────────────────────────────────────────────────╮
│  🏆 Top Skills                                               │
╰──────────────────────────────────────────────────────────────╯

  SKILL           USES    SMOOTH RATE
  /commit           18    ██████████████████  92%  ✓ keep using
  /brainstorm        7    █████████████████░  85%  ✓ keep using
  /debug             4    █████░░░░░░░░░░░░░  25%  ⚠ often turbulent

╭──────────────────────────────────────────────────────────────╮
│  💡 Insights                                                 │
╰──────────────────────────────────────────────────────────────╯

  • Your smoothest sessions start with /brainstorm
  • /debug sessions average 14 turns — consider breaking problems smaller
  • You shipped 3x more on Tue/Wed than other days

╭──────────────────────────────────────────────────────────────╮
│  🔥 Turbulent Sessions (might revisit)                       │
╰──────────────────────────────────────────────────────────────╯

  1. "Fix auth bug"         14 turns · 42min · 3 circle-backs
  2. "Refactor user model"  11 turns · 38min · 2 circle-backs

  Run: claude-view open <session-id> to review

──────────────────────────────────────────────────────────────────
  --day, --week, --month    Change time range
  --json                    Machine-readable output
  --no-color                Plain text mode
```

### 11.3 Narrow Terminal (< 60 chars)

```
📈 Jan 20-27
───────────────────────
Sessions     23  +5 ↑
Avg turns   4.2  ↓ better
Time to ship 12m ↓ faster
Rework       8%  ↓ cleaner

🏆 Top Skills
───────────────────────
/commit      18x  92% ✓
/brainstorm   7x  85% ✓
/debug        4x  25% ⚠
```

---

## 12. Insights Generation

### 12.1 Rule-Based Insights (MVP)

```rust
pub fn generate_insights(stats: &WeeklyStats) -> Vec<String> {
    let mut insights = vec![];

    // Skill correlation
    if let Some(best_starter) = stats.best_starting_skill() {
        insights.push(format!(
            "Your smoothest sessions start with {}",
            best_starter.name
        ));
    }

    // Turbulent skill warning
    for skill in &stats.skills {
        if skill.smooth_rate < 0.4 && skill.uses >= 3 {
            insights.push(format!(
                "{} sessions average {} turns — consider breaking problems smaller",
                skill.name, skill.avg_turns
            ));
        }
    }

    // Day-of-week pattern
    if let Some((best_days, ratio)) = stats.best_shipping_days() {
        if ratio >= 2.0 {
            insights.push(format!(
                "You shipped {}x more on {} than other days",
                ratio, best_days.join("/")
            ));
        }
    }

    insights
}
```

### 12.2 Future: LLM-Powered Insights

Post-MVP, can send anonymized stats to LLM for richer insights:
- "You tend to struggle with auth-related tasks"
- "Consider using /brainstorm before /debug sessions"
- "Your Tuesday productivity is 2x your Friday"

---

## 13. Implementation Phases

### Phase 2a: Core Metrics
- [ ] Add metrics columns to sessions table
- [ ] Calculate turn count, duration, circle-back rate on index
- [ ] Session health classification
- [ ] Stats API endpoint
- [ ] Dashboard stats banner

### Phase 2b: Git Correlation
- [ ] Git commit discovery (scan repos)
- [ ] Commit → session matching algorithm
- [ ] session_commits table
- [ ] Commits linked in session list UI
- [ ] "Possibly shipped" dropdown

### Phase 2c: CLI & Polish
- [ ] `claude-view stats` command
- [ ] Insights generation
- [ ] User correction endpoints
- [ ] Skill effectiveness ranking

---

## 14. Success Criteria

- [ ] Dashboard loads < 200ms for 1000+ sessions
- [ ] Git correlation accuracy > 80% (validated manually on 50 sessions)
- [ ] CLI output renders correctly in 80-char and 60-char terminals
- [ ] Stats aggregation handles 10k+ sessions without timeout
- [ ] "Turbulent" classification matches user intuition (spot check 20 sessions)

---

## 15. Open Questions

1. **Historical git data:** Should we scan git history on first run, or only track new commits going forward?
   - Recommendation: Scan last 30 days on first run, then incremental

2. **Multi-repo support:** User might have sessions spanning multiple git repos
   - Recommendation: Match session's project path to repo, support multiple repos

3. **Insight refresh frequency:** How often to regenerate insights?
   - Recommendation: On stats page load, cached for 1 hour

---

*2026-01-27*
