# Web (apps/web/) Plans

React SPA frontend — UI components, dashboard, session views.

## Active

| File | Status | Description |
|------|--------|-------------|
| `2026-03-02-web-auth-ux-design.md` | **done** (2026-03-02) | Auth pill + settings page — Supabase OAuth sign-in UX |
| `2026-03-02-web-auth-ux-impl.md` | **done** (2026-03-02) | AuthProvider, UserMenu, AccountSection, ConversationView refactor — 7 commits, 7 files |
| `2026-02-28-chat-input-bar-design.md` | **done** (2026-03-02) | Chat input + interactive message cards in monitor panels |
| `2026-02-28-chat-input-bar-impl.md` | **done** (2026-03-02) | 24 tasks, 7 commits — ChatInputBar, 4 interactive cards, ControlCallbacks, wired into RichPane + MonitorPane + SessionDetailPanel + ConversationView |
| `2026-03-02-share-viewer-upgrade-design.md` | **done** (2026-03-02) | Share viewer upgrade + share UX design |
| `2026-03-02-share-viewer-upgrade-impl.md` | **done** (2026-03-02) | Share viewer upgrade — 8 tasks, 7 commits, shippable audit passed |
| `2026-02-28-conversation-sharing-design.md` | done | Share conversations for education/demos |
| `2026-02-28-conversation-sharing-impl.md` | done (2026-03-01) | Implementation plan for conversation sharing |
| `2026-02-20-context-bar-threshold-marker.md` | **done** | Visual compaction threshold marker on context bar — 80% marker in ContextBar + ContextGauge, color zones unified (75/90), legend glyph in expanded mode |
| `2026-02-20-verbose-rich-renderers-design.md` | **done** | Rich renderers for tool_use inputs in verbose mode — ToolRenderers.tsx (12+ renderers: Edit/Write/Read/Grep/Glob/Bash/Task/Skill/WebFetch/WebSearch/Notebook/SmartMcp), per-card rich/json toggle, global toggle in ViewModeControls |
| `2026-02-24-star-label-sessions-design.md` | deferred (L0 nice-to-have) | Named bookmarks on sessions |

## Recently Completed

| File | Description |
|------|-------------|
| `2026-03-02-share-viewer-upgrade-impl.md` | Share Viewer Upgrade — ViewModeToggle + SessionInfoPanel shared components, verbose mode, redesigned header with backdrop-blur, ChatGPT-style share modal with Copy Link/Copy Message, expanded Rust share blob with rich metadata (8 tasks, 7 commits, shipped 2026-03-02) |
| `2026-03-02-share-viewer-upgrade-design.md` | Share Viewer Upgrade + Share UX design doc |
| `2026-02-28-chat-input-bar-impl.md` | ChatInputBar + Interactive Cards — dormant state machine, 4 interactive cards (AskUserQuestion, Permission, PlanApproval, Elicitation), ControlCallbacks dependency inversion, wired into SessionDetailPanel + ConversationView + RichPane (24 tasks, 7 commits, ~1200 lines added, shipped 2026-03-02) |
| `2026-02-28-chat-input-bar-design.md` | ChatInputBar + Interactive Cards design doc |
| `2026-03-02-web-auth-ux-impl.md` | Web Auth UX — AuthProvider context, UserMenu header avatar/dropdown, AccountSection settings, centralized sign-in modal via Radix Dialog (8 tasks, 7 commits, 7 files, shipped 2026-03-02) |
| `2026-03-02-web-auth-ux-design.md` | Web Auth UX design doc |
| `2026-02-28-conversation-sharing-impl.md` | Conversation sharing — AES-256-GCM encrypted links via Cloudflare Worker + R2 + D1, viewer SPA, share button + settings UI (9 tasks, 18 commits, shipped 2026-03-01) |
| `2026-02-28-conversation-sharing-design.md` | Conversation sharing design doc |
| `archived/2026-02-28-task-list-overview-design.md` | Task list display in live monitor |
| `archived/2026-02-28-task-list-overview-impl.md` | Task list implementation |
| `archived/2026-02-24-activity-dashboard-design.md` | Activity dashboard analytics |
| `archived/2026-02-24-activity-dashboard.md` | Activity dashboard implementation |
| `archived/2026-02-22-rich-tool-card-redesign-design.md` | Rich tool card visual redesign |
| `archived/2026-02-22-rich-tool-card-redesign.md` | Rich tool card implementation |
| `archived/2026-02-21-token-breakdown-cards-design.md` | Token breakdown card design |
| `archived/2026-02-21-token-breakdown-cards-impl.md` | Token breakdown card implementation |
| `archived/2026-02-21-report-details-design.md` | Report details panel design |
| `archived/2026-02-21-report-details-plan.md` | Report details implementation |
| `archived/2026-02-21-report-ui-plan.md` | Report UI plan |
| `archived/2026-02-20-history-detail-compact-verbose-design.md` | History detail compact/verbose toggle |
| `archived/2026-02-20-history-detail-compact-verbose.md` | History detail implementation |
| `archived/2026-02-19-action-log-tab-design.md` | Filterable action timeline |
| `archived/2026-02-19-action-log-tab-impl.md` | Action log implementation |
| `archived/2026-02-19-notification-sound-design.md` | Audio notifications |
| `archived/2026-02-19-notification-sound-impl.md` | Notification sound implementation |
| `archived/2026-02-19-oauth-usage-pill-design.md` | OAuth usage pill |
| `archived/2026-02-19-restore-sparkline-stats-grid.md` | Sparkline stats grid |
| `archived/2026-02-19-sessions-infinite-scroll.md` | Infinite scroll for session lists |
| `archived/2026-02-05-theme4-chat-insights-design.md` | Theme 4 chat insights master design |

## Backlog

(Area-specific future work)
