use std::sync::Arc;

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;
use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", ApiDoc::openapi()))
}
