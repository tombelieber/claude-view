//! Collects team-related entries across JSONL lines and assembles a
//! `TeamTranscriptBlock` at finalization.

use crate::block_types::{TeamTranscriptBlock, TranscriptEntry, TranscriptSpeaker};
use crate::transcript::{self, TeammateContent};

/// Accumulates team-related entries across JSONL lines.
pub struct TranscriptBuilder {
    team_name: String,
    description: String,
    speakers: Vec<TranscriptSpeaker>,
    entries: Vec<TranscriptEntry>,
    speaker_ids: std::collections::HashSet<String>,
}

impl TranscriptBuilder {
    pub fn new(team_name: String, description: String) -> Self {
        Self {
            team_name,
            description,
            speakers: Vec::new(),
            entries: Vec::new(),
            speaker_ids: std::collections::HashSet::new(),
        }
    }

    /// Parse `<teammate-message>` blocks from a user-role string.
    pub fn add_teammate_messages(&mut self, text: &str, line_index: usize) {
        let parsed = transcript::parse_teammate_messages(text);
        for (i, msg) in parsed.into_iter().enumerate() {
            // Auto-register speaker on first appearance
            if !msg.teammate_id.is_empty()
                && msg.teammate_id != "system"
                && self.speaker_ids.insert(msg.teammate_id.clone())
            {
                self.speakers.push(TranscriptSpeaker {
                    id: msg.teammate_id.clone(),
                    display_name: transcript::make_display_name(&msg.teammate_id),
                    color: msg.color.clone(),
                    stance: None,
                });
            }

            match msg.content {
                TeammateContent::Message(text) => {
                    self.entries.push(TranscriptEntry::AgentMessage {
                        teammate_id: msg.teammate_id,
                        color: msg.color,
                        summary: msg.summary,
                        text,
                        line_index: line_index + i,
                    });
                }
                TeammateContent::Protocol { msg_type, raw } => {
                    self.entries.push(TranscriptEntry::Protocol {
                        teammate_id: msg.teammate_id,
                        msg_type,
                        raw,
                        line_index: line_index + i,
                    });
                }
            }
        }
    }

    /// Add moderator narration (assistant text block with teamName set).
    pub fn add_moderator_narration(&mut self, text: String, line_index: usize) {
        self.entries.push(TranscriptEntry::ModeratorNarration {
            text,
            is_verdict: false,
            line_index,
        });
    }

    /// Mark the last moderator narration as the verdict.
    pub fn mark_verdict(&mut self) {
        for entry in self.entries.iter_mut().rev() {
            if matches!(entry, TranscriptEntry::ModeratorNarration { .. }) {
                if let TranscriptEntry::ModeratorNarration { is_verdict, .. } = entry {
                    *is_verdict = true;
                }
                break;
            }
        }
    }

    /// Add a moderator relay message (from SendMessage tool_use).
    pub fn add_moderator_relay(&mut self, to: String, message: String, line_index: usize) {
        self.entries.push(TranscriptEntry::ModeratorRelay {
            to,
            message,
            line_index,
        });
    }

    /// Add a task event (from TaskCreate/TaskUpdate tool_use).
    pub fn add_task_event(
        &mut self,
        subject: String,
        status: Option<String>,
        owner: Option<String>,
        line_index: usize,
    ) {
        self.entries.push(TranscriptEntry::TaskEvent {
            subject,
            status,
            owner,
            line_index,
        });
    }

    /// Add a team lifecycle event.
    pub fn add_lifecycle(&mut self, event: String, line_index: usize) {
        self.entries
            .push(TranscriptEntry::TeamLifecycle { event, line_index });
    }

    /// Register a speaker from Agent spawn data.
    pub fn add_speaker_from_spawn(&mut self, id: &str, color: Option<&str>, description: &str) {
        let stance = extract_stance(description);

        if self.speaker_ids.insert(id.to_string()) {
            self.speakers.push(TranscriptSpeaker {
                id: id.to_string(),
                display_name: transcript::make_display_name(id),
                color: color.map(String::from),
                stance,
            });
        } else {
            // Update existing speaker with stance if we didn't have it
            if let Some(speaker) = self.speakers.iter_mut().find(|s| s.id == id) {
                if speaker.stance.is_none() {
                    speaker.stance = stance;
                }
                if speaker.color.is_none() {
                    speaker.color = color.map(String::from);
                }
            }
        }
    }

    /// Sort entries by line_index and produce the final block.
    pub fn build(mut self, id: String) -> TeamTranscriptBlock {
        self.entries.sort_by_key(|e| match e {
            TranscriptEntry::AgentMessage { line_index, .. }
            | TranscriptEntry::ModeratorNarration { line_index, .. }
            | TranscriptEntry::ModeratorRelay { line_index, .. }
            | TranscriptEntry::TaskEvent { line_index, .. }
            | TranscriptEntry::TeamLifecycle { line_index, .. }
            | TranscriptEntry::Protocol { line_index, .. } => *line_index,
        });

        TeamTranscriptBlock {
            id,
            team_name: self.team_name,
            description: self.description,
            speakers: self.speakers,
            entries: self.entries,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Extract stance from Agent spawn description.
/// "Debate: argue FOR tabs" → "Argues FOR tabs"
/// "Debate: argue AGAINST AI" → "Argues AGAINST AI"
/// Unrecognized patterns → None
fn extract_stance(desc: &str) -> Option<String> {
    let lower = desc.to_lowercase();
    if let Some(pos) = lower.find("argue ") {
        let rest = &desc[pos + 6..]; // skip "argue "
        Some(format!("Argues {rest}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_agent_messages() {
        let mut builder = TranscriptBuilder::new("debate".into(), "Tabs vs spaces".into());
        builder.add_teammate_messages(
            r#"<teammate-message teammate_id="tabs" color="blue" summary="Opening">
Tabs are better.
</teammate-message>"#,
            5,
        );
        let block = builder.build("tt-1".into());
        assert_eq!(block.speakers.len(), 1);
        assert_eq!(block.speakers[0].id, "tabs");
        assert_eq!(block.speakers[0].display_name, "Tabs");
        assert_eq!(block.entries.len(), 1);
        assert!(
            matches!(&block.entries[0], TranscriptEntry::AgentMessage { text, .. } if text.contains("Tabs are better"))
        );
    }

    #[test]
    fn protocol_messages_separated() {
        let mut builder = TranscriptBuilder::new("debate".into(), "Test".into());
        builder.add_teammate_messages(
            r#"<teammate-message teammate_id="agent" color="green" summary="Argument">
Good argument here.
</teammate-message>

<teammate-message teammate_id="agent" color="green">
{"type":"idle_notification","from":"agent","timestamp":"2026-04-06T17:11:53.582Z","idleReason":"available"}
</teammate-message>"#,
            10,
        );
        let block = builder.build("tt-1".into());
        assert_eq!(block.entries.len(), 2);
        assert!(matches!(
            &block.entries[0],
            TranscriptEntry::AgentMessage { .. }
        ));
        assert!(matches!(
            &block.entries[1],
            TranscriptEntry::Protocol { .. }
        ));
    }

    #[test]
    fn moderator_narration_added() {
        let mut builder = TranscriptBuilder::new("debate".into(), "Test".into());
        builder.add_moderator_narration("Strong openings! Round 2.".into(), 15);
        let block = builder.build("tt-1".into());
        assert_eq!(block.entries.len(), 1);
        assert!(matches!(
            &block.entries[0],
            TranscriptEntry::ModeratorNarration {
                is_verdict: false,
                ..
            }
        ));
    }

    #[test]
    fn last_narration_before_build_is_verdict() {
        let mut builder = TranscriptBuilder::new("debate".into(), "Test".into());
        builder.add_moderator_narration("Round 1 summary.".into(), 10);
        builder.add_moderator_narration("Final verdict here.".into(), 20);
        builder.mark_verdict();
        let block = builder.build("tt-1".into());
        assert_eq!(block.entries.len(), 2);
        assert!(matches!(
            &block.entries[0],
            TranscriptEntry::ModeratorNarration {
                is_verdict: false,
                ..
            }
        ));
        assert!(matches!(
            &block.entries[1],
            TranscriptEntry::ModeratorNarration {
                is_verdict: true,
                ..
            }
        ));
    }

    #[test]
    fn relay_and_task_events() {
        let mut builder = TranscriptBuilder::new("debate".into(), "Test".into());
        builder.add_moderator_relay("tabs".into(), "Here's what spaces said...".into(), 12);
        builder.add_task_event(
            "Opening argument".into(),
            Some("completed".into()),
            Some("tabs".into()),
            8,
        );
        let block = builder.build("tt-1".into());
        // Entries sorted by line_index: task(8), relay(12)
        assert_eq!(block.entries.len(), 2);
        assert!(matches!(
            &block.entries[0],
            TranscriptEntry::TaskEvent { .. }
        ));
        assert!(matches!(
            &block.entries[1],
            TranscriptEntry::ModeratorRelay { .. }
        ));
    }

    #[test]
    fn speaker_stance_from_agent_description() {
        let mut builder = TranscriptBuilder::new("debate".into(), "Test".into());
        builder.add_speaker_from_spawn("pro-ai", Some("blue"), "Debate: argue FOR tabs");
        let block = builder.build("tt-1".into());
        assert_eq!(block.speakers[0].stance.as_deref(), Some("Argues FOR tabs"));
    }

    #[test]
    fn empty_builder_produces_empty_transcript() {
        let builder = TranscriptBuilder::new("debate".into(), "Test".into());
        let block = builder.build("tt-1".into());
        assert!(block.speakers.is_empty());
        assert!(block.entries.is_empty());
    }
}
