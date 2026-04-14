//! Optional OTLP exporter, gated behind `otel` feature.
//!
//! When enabled, builds an `SdkTracerProvider` that ships spans over gRPC
//! (tonic) to any OTLP-compatible collector (Jaeger, Tempo, etc.).
//!
//! The endpoint can be set:
//! 1. Via the standard `OTEL_EXPORTER_OTLP_ENDPOINT` env var (OTel SDK reads it automatically).
//! 2. Via `CLAUDE_VIEW_OTLP_ENDPOINT` env var (read by `ServiceConfig`).
//! 3. Programmatically through `build_tracer_provider`.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;

/// Build an `SdkTracerProvider` that exports spans via OTLP/gRPC.
///
/// If `endpoint` is `Some`, it is set on the exporter builder. Otherwise the
/// OTel SDK falls back to `OTEL_EXPORTER_OTLP_ENDPOINT` or the default
/// `http://localhost:4317`.
pub fn build_tracer_provider(
    service_name: &str,
    service_version: &str,
    endpoint: Option<&str>,
) -> anyhow::Result<SdkTracerProvider> {
    let mut builder = SpanExporter::builder().with_tonic();

    if let Some(ep) = endpoint {
        builder = builder.with_endpoint(ep);
    }

    let exporter = builder
        .build()
        .map_err(|e| anyhow::anyhow!("OTel OTLP exporter: {e}"))?;

    let resource = Resource::builder()
        .with_service_name(service_name.to_owned())
        .with_attribute(opentelemetry::KeyValue::new(
            "service.version",
            service_version.to_owned(),
        ))
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    Ok(provider)
}

/// Build a type-erased `tracing_opentelemetry` layer from an existing provider.
///
/// Returns a `Box<dyn Layer<S>>` so it composes with any subscriber stack
/// (the concrete `OpenTelemetryLayer<S, T>` is `Layer<S>` only for the `S` it
/// was created with, which causes type mismatch when `.with()` nests layers).
pub fn build_layer<S>(
    provider: &SdkTracerProvider,
) -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber
        + for<'span> tracing_subscriber::registry::LookupSpan<'span>
        + Send
        + Sync,
{
    Box::new(tracing_opentelemetry::OpenTelemetryLayer::new(
        provider.tracer("claude-view"),
    ))
}
