//! Progress item processing (TodoWrite, TaskCreate, TaskIdAssignment, TaskUpdate).

use super::super::accumulator::SessionAccumulator;

pub(super) fn process_progress_items(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
) {
    // TodoWrite: full replacement
    if let Some(ref todos) = line.todo_write {
        use claude_view_core::progress::{ProgressItem, ProgressSource, ProgressStatus};
        acc.todo_items = todos
            .iter()
            .map(|t| {
                let status = match t.status.as_str() {
                    "in_progress" => ProgressStatus::InProgress,
                    "completed" => ProgressStatus::Completed,
                    _ => ProgressStatus::Pending,
                };
                ProgressItem {
                    id: None,
                    tool_use_id: None,
                    title: t.content.clone(),
                    status,
                    active_form: if t.active_form.is_empty() {
                        None
                    } else {
                        Some(t.active_form.clone())
                    },
                    source: ProgressSource::Todo,
                    description: None,
                }
            })
            .collect();
    }

    // TaskCreate: append with dedup guard
    for create in &line.task_creates {
        use claude_view_core::progress::{ProgressItem, ProgressSource, ProgressStatus};
        if acc
            .task_items
            .iter()
            .any(|t| t.tool_use_id.as_deref() == Some(&create.tool_use_id))
        {
            continue;
        }
        acc.task_items.push(ProgressItem {
            id: None,
            tool_use_id: Some(create.tool_use_id.clone()),
            title: create.subject.clone(),
            status: ProgressStatus::Pending,
            active_form: if create.active_form.is_empty() {
                None
            } else {
                Some(create.active_form.clone())
            },
            source: ProgressSource::Task,
            description: if create.description.is_empty() {
                None
            } else {
                Some(create.description.clone())
            },
        });
    }

    // TaskIdAssignment: assign system ID
    for assignment in &line.task_id_assignments {
        if let Some(task) = acc
            .task_items
            .iter_mut()
            .find(|t| t.tool_use_id.as_deref() == Some(&assignment.tool_use_id))
        {
            task.id = Some(assignment.task_id.clone());
        }
    }

    // TaskUpdate: modify existing task
    for update in &line.task_updates {
        use claude_view_core::progress::ProgressStatus;
        if let Some(task) = acc
            .task_items
            .iter_mut()
            .find(|t| t.id.as_deref() == Some(&update.task_id))
        {
            if let Some(ref s) = update.status {
                task.status = match s.as_str() {
                    "in_progress" => ProgressStatus::InProgress,
                    "completed" => ProgressStatus::Completed,
                    _ => ProgressStatus::Pending,
                };
            }
            if let Some(ref subj) = update.subject {
                task.title = subj.clone();
            }
            if let Some(ref af) = update.active_form {
                task.active_form = Some(af.clone());
            }
        }
    }
}
