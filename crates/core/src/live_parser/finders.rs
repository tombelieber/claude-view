//! SIMD-accelerated substring finders for pre-filtering JSONL lines.

use memchr::memmem;

/// Pre-compiled SIMD substring finders. Build once at startup via
/// [`TailFinders::new`] and share across polls.
pub struct TailFinders {
    pub content_key: memmem::Finder<'static>,
    pub model_key: memmem::Finder<'static>,
    pub usage_key: memmem::Finder<'static>,
    pub tool_use_key: memmem::Finder<'static>,
    pub name_key: memmem::Finder<'static>,
    pub stop_reason_key: memmem::Finder<'static>,
    pub task_name_key: memmem::Finder<'static>,
    pub agent_name_key: memmem::Finder<'static>,
    pub tool_use_result_key: memmem::Finder<'static>,
    pub agent_progress_key: memmem::Finder<'static>,
    pub todo_write_key: memmem::Finder<'static>,
    pub task_create_key: memmem::Finder<'static>,
    pub task_update_key: memmem::Finder<'static>,
    pub task_notification_key: memmem::Finder<'static>,
    pub compact_boundary_key: memmem::Finder<'static>,
    pub hook_progress_key: memmem::Finder<'static>,
    pub at_file_key: memmem::Finder<'static>,
}

impl TailFinders {
    /// Create all finders once. The needles are `'static` string slices.
    pub fn new() -> Self {
        Self {
            content_key: memmem::Finder::new(b"\"content\""),
            model_key: memmem::Finder::new(b"\"model\""),
            usage_key: memmem::Finder::new(b"\"usage\""),
            tool_use_key: memmem::Finder::new(b"\"tool_use\""),
            name_key: memmem::Finder::new(b"\"name\""),
            stop_reason_key: memmem::Finder::new(b"\"stop_reason\""),
            task_name_key: memmem::Finder::new(b"\"name\":\"Task\""),
            agent_name_key: memmem::Finder::new(b"\"name\":\"Agent\""),
            tool_use_result_key: memmem::Finder::new(b"\"toolUseResult\""),
            agent_progress_key: memmem::Finder::new(b"\"agent_progress\""),
            todo_write_key: memmem::Finder::new(b"\"name\":\"TodoWrite\""),
            task_create_key: memmem::Finder::new(b"\"name\":\"TaskCreate\""),
            task_update_key: memmem::Finder::new(b"\"name\":\"TaskUpdate\""),
            task_notification_key: memmem::Finder::new(b"<task-notification>"),
            compact_boundary_key: memmem::Finder::new(b"\"compact_boundary\""),
            hook_progress_key: memmem::Finder::new(b"\"hook_progress\""),
            at_file_key: memmem::Finder::new(b"@"),
        }
    }
}

impl Default for TailFinders {
    fn default() -> Self {
        Self::new()
    }
}
