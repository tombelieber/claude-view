use crate::correlation::RequestId;
use http::{HeaderName, HeaderValue, Request};
use tower_http::request_id::{
    MakeRequestId, PropagateRequestIdLayer, RequestId as TowerRequestId, SetRequestIdLayer,
};

static HEADER: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Default)]
struct UlidRequestId;

impl MakeRequestId for UlidRequestId {
    fn make_request_id<B>(&mut self, _req: &Request<B>) -> Option<TowerRequestId> {
        let id = RequestId::new().to_string();
        HeaderValue::from_str(&id).ok().map(TowerRequestId::new)
    }
}

pub fn apply_request_id_layers<S>(router: axum::Router<S>) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(PropagateRequestIdLayer::new(HEADER.clone()))
        .layer(SetRequestIdLayer::new(HEADER.clone(), UlidRequestId))
}
