use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Swagger UI + OpenAPI spec endpoint.
///
/// Feature-gated behind `swagger` — excluded from dist builds to save ~3-4MB
/// of embedded JS/CSS assets. `utoipa::ToSchema` derives remain (zero-cost),
/// only the web UI is gated.
#[cfg(feature = "swagger")]
pub fn router() -> Router<Arc<AppState>> {
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    use crate::openapi::ApiDoc;

    Router::new().merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", ApiDoc::openapi()))
}

#[cfg(not(feature = "swagger"))]
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
}
