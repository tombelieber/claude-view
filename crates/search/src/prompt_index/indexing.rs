//! Index lifecycle: open, create, index documents, commit, version sync.

use std::path::Path;
use std::sync::Mutex;

use tantivy::{doc, Index, ReloadPolicy};

use claude_view_core::prompt_templates::normalize_to_template;

use crate::SearchError;

use super::types::{
    build_prompt_schema, fxhash, PromptDocument, PromptSearchIndex, PROMPT_SCHEMA_VERSION,
};

impl PromptSearchIndex {
    /// Open or create a prompt index at the given path.
    /// Schema version mismatch triggers a full wipe and rebuild.
    pub fn open(path: &Path) -> Result<Self, SearchError> {
        std::fs::create_dir_all(path)?;

        let version_path = path.join("schema_version");
        let needs_rebuild = match std::fs::read_to_string(&version_path) {
            Ok(v) => v.trim().parse::<u32>().unwrap_or(0) != PROMPT_SCHEMA_VERSION,
            Err(_) => true,
        };

        if needs_rebuild {
            tracing::info!(
                path = %path.display(),
                "Prompt index schema version mismatch — rebuilding"
            );
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.file_name()
                        .map(|n| n != "schema_version")
                        .unwrap_or(false)
                    {
                        if p.is_dir() {
                            let _ = std::fs::remove_dir_all(&p);
                        } else {
                            let _ = std::fs::remove_file(&p);
                        }
                    }
                }
            }
        }

        let schema = build_prompt_schema();
        let index = match Index::open_in_dir(path) {
            Ok(idx) => {
                tracing::info!(path = %path.display(), "opened existing prompt index");
                idx
            }
            Err(_) => {
                tracing::info!(path = %path.display(), "creating new prompt index");
                Index::create_in_dir(path, schema.clone())?
            }
        };

        Self::from_index(index, schema, needs_rebuild, Some(version_path))
    }

    /// Create a prompt index in RAM (for tests).
    pub fn open_in_ram() -> Result<Self, SearchError> {
        let schema = build_prompt_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, schema, false, None)
    }

    /// Internal: set up reader, writer, and field handles from an Index + Schema.
    fn from_index(
        index: Index,
        schema: tantivy::schema::Schema,
        needs_full_reindex: bool,
        version_file_path: Option<std::path::PathBuf>,
    ) -> Result<Self, SearchError> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let writer = index.writer(50_000_000)?;

        let prompt_id_field = schema.get_field("prompt_id").expect("missing prompt_id");
        let display_field = schema.get_field("display").expect("missing display");
        let paste_text_field = schema.get_field("paste_text").expect("missing paste_text");
        let project_field = schema.get_field("project").expect("missing project");
        let session_id_field = schema.get_field("session_id").expect("missing session_id");
        let branch_field = schema.get_field("branch").expect("missing branch");
        let model_field = schema.get_field("model").expect("missing model");
        let git_root_field = schema.get_field("git_root").expect("missing git_root");
        let intent_field = schema.get_field("intent").expect("missing intent");
        let complexity_field = schema.get_field("complexity").expect("missing complexity");
        let timestamp_field = schema.get_field("timestamp").expect("missing timestamp");
        let has_paste_field = schema.get_field("has_paste").expect("missing has_paste");
        let template_id_field = schema
            .get_field("template_id")
            .expect("missing template_id");
        let is_template_field = schema
            .get_field("is_template")
            .expect("missing is_template");

        Ok(Self {
            index,
            reader,
            writer: Mutex::new(writer),
            schema,
            needs_full_reindex,
            version_file_path,
            prompt_id_field,
            display_field,
            paste_text_field,
            project_field,
            session_id_field,
            branch_field,
            model_field,
            git_root_field,
            intent_field,
            complexity_field,
            timestamp_field,
            has_paste_field,
            template_id_field,
            is_template_field,
        })
    }

    /// Bulk-index prompt documents. History is append-only, no deletes needed.
    pub fn index_prompts(&self, docs: &[PromptDocument]) -> Result<(), SearchError> {
        let writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(format!("writer lock poisoned: {e}")))
        })?;
        for d in docs {
            let mut tantivy_doc = doc!(
                self.prompt_id_field => d.prompt_id.as_str(),
                self.display_field => d.display.as_str(),
                self.project_field => d.project.as_str(),
                self.session_id_field => d.session_id.as_deref().unwrap_or(""),
                self.branch_field => d.branch.as_str(),
                self.model_field => d.model.as_str(),
                self.git_root_field => d.git_root.as_str(),
                self.intent_field => d.intent.as_str(),
                self.complexity_field => d.complexity.as_str(),
                self.timestamp_field => d.timestamp,
                self.has_paste_field => if d.has_paste { "true" } else { "false" },
            );
            if let Some(ref paste) = d.paste_text {
                tantivy_doc.add_text(self.paste_text_field, paste);
            }
            // Compute template classification: normalize display text to detect slots.
            // template_id field stores the stable hash of the normalized pattern (empty = unique).
            // is_template field stores "true"/"false" for fast TermQuery filtering.
            let normalized = normalize_to_template(&d.display);
            let (template_id_val, is_template_val) = if normalized != d.display {
                (format!("{:x}", fxhash(normalized.as_bytes())), "true")
            } else {
                (String::new(), "false")
            };
            tantivy_doc.add_text(self.template_id_field, &template_id_val);
            tantivy_doc.add_text(self.is_template_field, is_template_val);
            writer.add_document(tantivy_doc)?;
        }
        Ok(())
    }

    /// Commit pending writes to disk.
    /// Call this after indexing a batch of prompts.
    pub fn commit(&self) -> Result<(), SearchError> {
        let mut writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(format!("writer lock poisoned: {e}")))
        })?;
        writer.commit()?;
        self.reader.reload()?;
        tracing::info!("prompt index committed");
        Ok(())
    }

    /// Write schema version to disk after successful indexing.
    pub fn mark_schema_synced(&self) {
        if let Some(path) = &self.version_file_path {
            if let Err(e) = std::fs::write(path, format!("{PROMPT_SCHEMA_VERSION}")) {
                tracing::warn!(error = %e, "Failed to write prompt schema version file");
            } else {
                tracing::info!(
                    version = PROMPT_SCHEMA_VERSION,
                    "Prompt schema version synced"
                );
            }
        }
    }
}
