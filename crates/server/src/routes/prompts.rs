use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_search::prompt_index::PromptSearchParams;
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
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
    pub template_match: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    pub id: String,
    pub display: String,
    /// HTML snippet with `<b>` tags around matched search terms.
    /// `None` in browse/filter-only mode (no free-text query).
    pub snippet: Option<String>,
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

    // Build search query from params — free-text + qualifier tokens
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
    let query_str = query_parts.join(" ");

    // Parse has_paste param: "true" → Some(true), "false" → Some(false), absent → None
    let has_paste_filter = match params.has_paste.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };

    let search_params = PromptSearchParams {
        query: &query_str,
        scope: params.project.as_deref(),
        has_paste: has_paste_filter,
        time_after: params.time_after,
        time_before: params.time_before,
        sort: params.sort.as_deref(),
        template_match: params.template_match.as_deref(),
        limit,
        offset,
    };

    match index.search_with(search_params) {
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
                        snippet: h.snippet,
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
                        template_id: h.template_id,
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

    #[test]
    fn parse_prompts_query_new_fields() {
        let q: PromptsListQuery =
            serde_json::from_str(r#"{"time_after":1700000000,"time_before":1800000000,"has_paste":"true","sort":"oldest"}"#)
                .unwrap();
        assert_eq!(q.time_after, Some(1700000000));
        assert_eq!(q.time_before, Some(1800000000));
        assert_eq!(q.has_paste.as_deref(), Some("true"));
        assert_eq!(q.sort.as_deref(), Some("oldest"));
    }

    #[test]
    fn template_match_param_is_passed_to_search_params() {
        // Verify that `template_match` query param round-trips through PromptsListQuery
        // and is correctly forwarded to PromptSearchParams.
        let q: PromptsListQuery = serde_json::from_str(r#"{"template_match":"template"}"#).unwrap();
        assert_eq!(q.template_match.as_deref(), Some("template"));

        // Build the PromptSearchParams as the handler does and confirm template_match is set.
        let search_params = PromptSearchParams {
            query: "",
            scope: None,
            has_paste: None,
            time_after: None,
            time_before: None,
            sort: None,
            template_match: q.template_match.as_deref(),
            limit: 20,
            offset: 0,
        };
        assert_eq!(search_params.template_match, Some("template"));
    }
}
