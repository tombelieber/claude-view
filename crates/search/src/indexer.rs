use tantivy::doc;
use tantivy::Term;
use tracing::{debug, info};

use crate::{SearchError, SearchIndex};

/// A document to be indexed in Tantivy, representing a single message
/// from a Claude Code conversation.
pub struct SearchDocument {
    pub session_id: String,
    pub project: String,
    /// Use "" if the session has no branch.
    pub branch: String,
    /// Use "" if the session has no model.
    pub model: String,
    /// "user", "assistant", or "tool"
    pub role: String,
    /// The message text content.
    pub content: String,
    /// 1-based turn number.
    pub turn_number: u64,
    /// Unix timestamp in seconds. 0 if unknown.
    pub timestamp: i64,
    /// Skills used in this message (multi-valued).
    pub skills: Vec<String>,
}

impl SearchIndex {
    /// Index all messages for a session. Deletes any existing documents for this
    /// session_id first, then adds the new documents. Does NOT commit —
    /// call `commit()` after indexing a batch.
    pub fn index_session(
        &self,
        session_id: &str,
        docs: &[SearchDocument],
    ) -> Result<(), SearchError> {
        let writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(
                format!("writer lock poisoned: {e}"),
            ))
        })?;

        // Delete all existing documents for this session
        let delete_term = Term::from_field_text(self.session_id_field, session_id);
        writer.delete_term(delete_term);

        // Add new documents
        for doc_data in docs {
            let mut tantivy_doc = doc!(
                self.session_id_field => doc_data.session_id.as_str(),
                self.project_field => doc_data.project.as_str(),
                self.branch_field => doc_data.branch.as_str(),
                self.model_field => doc_data.model.as_str(),
                self.role_field => doc_data.role.as_str(),
                self.content_field => doc_data.content.as_str(),
                self.turn_number_field => doc_data.turn_number,
                self.timestamp_field => doc_data.timestamp,
            );

            // Add each skill as a separate value (multi-valued field)
            for skill in &doc_data.skills {
                tantivy_doc.add_text(self.skills_field, skill);
            }

            writer.add_document(tantivy_doc)?;
        }

        debug!(
            session_id = session_id,
            doc_count = docs.len(),
            "indexed session documents"
        );

        Ok(())
    }

    /// Delete all documents for a given session_id. Does NOT commit.
    pub fn delete_session(&self, session_id: &str) -> Result<(), SearchError> {
        let writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(
                format!("writer lock poisoned: {e}"),
            ))
        })?;

        let delete_term = Term::from_field_text(self.session_id_field, session_id);
        writer.delete_term(delete_term);

        debug!(session_id = session_id, "deleted session from search index");

        Ok(())
    }

    /// Commit all pending writes (inserts and deletes) to disk.
    /// Call this after indexing a batch of sessions.
    pub fn commit(&self) -> Result<(), SearchError> {
        let mut writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(
                format!("writer lock poisoned: {e}"),
            ))
        })?;

        writer.commit()?;
        info!("search index committed");

        Ok(())
    }
}
