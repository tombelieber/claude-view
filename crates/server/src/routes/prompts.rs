use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ts_rs::TS;

use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct PromptsListQuery {
    pub q: Option<String>,
    pub project: Option<String>,
    pub intent: Option<String>,
    pub complexity: Option<String>,
    pub has_paste: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    pub id: String,
    pub display: String,
    pub project: String,
    pub project_display_name: String,
    pub session_id: Option<String>,
    pub timestamp: i64,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub intent: String,
    pub complexity: String,
    pub has_paste: bool,
    pub paste_preview: Option<String>,
    pub template_id: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptListResponse {
    pub prompts: Vec<PromptInfo>,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateInfo {
    pub pattern: String,
    pub frequency: usize,
    pub examples: Vec<String>,
    pub slots: Vec<String>,
}

async fn list_prompts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PromptsListQuery>,
) -> Json<PromptListResponse> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let index_guard = state.prompt_index.read().unwrap();
    let Some(index) = index_guard.as_ref() else {
        return Json(PromptListResponse {
            prompts: vec![],
            total: 0,
            has_more: false,
        });
    };

    // Build search query from params
    let mut query_parts = Vec::new();
    if let Some(ref q) = params.q {
        query_parts.push(q.clone());
    }
    if let Some(ref intent) = params.intent {
        query_parts.push(format!("intent:{intent}"));
    }
    if let Some(ref complexity) = params.complexity {
        query_parts.push(format!("complexity:{complexity}"));
    }

    let query_str = if query_parts.is_empty() {
        String::new()
    } else {
        query_parts.join(" ")
    };

    let scope = params.project.as_deref();

    match index.search(&query_str, scope, limit, offset) {
        Ok(result) => {
            let prompts: Vec<PromptInfo> = result
                .prompts
                .into_iter()
                .map(|h| {
                    let project_display = h
                        .project
                        .rsplit('/')
                        .next()
                        .unwrap_or(&h.project)
                        .to_string();
                    let branch = if h.branch.is_empty() {
                        None
                    } else {
                        Some(h.branch)
                    };
                    let model = if h.model.is_empty() {
                        None
                    } else {
                        Some(h.model)
                    };
                    PromptInfo {
                        id: h.prompt_id,
                        display: h.display,
                        project: h.project,
                        project_display_name: project_display,
                        session_id: h.session_id,
                        timestamp: h.timestamp,
                        branch,
                        model,
                        intent: h.intent,
                        complexity: h.complexity,
                        has_paste: h.has_paste,
                        paste_preview: None,
                        template_id: None,
                    }
                })
                .collect();
            let total = result.total_matches;
            Json(PromptListResponse {
                has_more: offset + prompts.len() < total,
                prompts,
                total,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "prompt search failed");
            Json(PromptListResponse {
                prompts: vec![],
                total: 0,
                has_more: false,
            })
        }
    }
}

async fn get_prompt_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Option<claude_view_core::prompt_history::PromptStats>> {
    let guard = state.prompt_stats.read().unwrap();
    Json(guard.clone())
}

async fn get_prompt_templates(State(state): State<Arc<AppState>>) -> Json<Vec<PromptTemplateInfo>> {
    let guard = state.prompt_templates.read().unwrap();
    match guard.as_ref() {
        Some(templates) => {
            let infos: Vec<PromptTemplateInfo> = templates
                .iter()
                .map(|t| PromptTemplateInfo {
                    pattern: t.pattern.clone(),
                    frequency: t.frequency,
                    examples: t.examples.clone(),
                    slots: t.slots.clone(),
                })
                .collect();
            Json(infos)
        }
        None => Json(vec![]),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/prompts", get(list_prompts))
        .route("/prompts/stats", get(get_prompt_stats))
        .route("/prompts/templates", get(get_prompt_templates))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prompts_query_defaults() {
        let q: PromptsListQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.limit, None);
        assert_eq!(q.sort, None);
        assert!(q.intent.is_none());
    }
}
