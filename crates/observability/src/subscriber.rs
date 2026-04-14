use crate::config::{ServiceConfig, SinkMode};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

pub struct ObservabilityHandle {
    _appender_guard: Option<WorkerGuard>,
    _sentry_guard: Option<crate::sentry_integration::SentryGuard>,
    #[cfg(feature = "otel")]
    _otel_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl ObservabilityHandle {
    pub fn shutdown(self) {
        #[cfg(feature = "otel")]
        if let Some(provider) = &self._otel_provider {
            if let Err(e) = provider.shutdown() {
                eprintln!("OTel provider shutdown error: {e}");
            }
        }
        drop(self);
    }
}

pub fn init(cfg: ServiceConfig) -> anyhow::Result<ObservabilityHandle> {
    std::fs::create_dir_all(&cfg.log_dir)?;

    let sentry_guard = crate::sentry_integration::init_if_enabled(
        cfg.sentry_dsn.clone(),
        cfg.service_name,
        &cfg.deployment_mode,
    );

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cfg.default_filter));

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix(format!("{}-", cfg.service_name))
        .filename_suffix("jsonl")
        .max_log_files(168)
        .build(&cfg.log_dir)
        .map_err(|e| anyhow::anyhow!("rolling file appender: {e}"))?;
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let json_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_writer(non_blocking);

    let dev_layer = match cfg.sink_mode {
        SinkMode::Both | SinkMode::DevOnly => Some(
            fmt::layer()
                .with_target(true)
                .with_level(true)
                .with_thread_names(true)
                .with_span_events(fmt::format::FmtSpan::CLOSE)
                .compact()
                .with_writer(std::io::stderr),
        ),
        SinkMode::ProdOnly => None,
    };

    let file_layer = match cfg.sink_mode {
        SinkMode::Both | SinkMode::ProdOnly => Some(json_layer),
        SinkMode::DevOnly => None,
    };

    let sentry_layer = sentry_guard.as_ref().map(|_| sentry_tracing::layer());

    // OTel layer: only built when the `otel` feature is enabled AND an endpoint is configured.
    #[cfg(feature = "otel")]
    let (otel_layer, otel_provider) = match cfg.otel_endpoint.as_deref() {
        Some(ep) => {
            let provider = crate::otel::build_tracer_provider(
                cfg.service_name,
                cfg.service_version,
                Some(ep),
            )?;
            let layer = crate::otel::build_layer(&provider);
            (Some(layer), Some(provider))
        }
        None => (None, None),
    };
    #[cfg(not(feature = "otel"))]
    let otel_layer: Option<tracing_subscriber::layer::Identity> = None;

    Registry::default()
        .with(filter)
        .with(dev_layer)
        .with(file_layer)
        .with(sentry_layer)
        .with(otel_layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("tracing subscriber init: {e}"))?;

    crate::panic_hook::install(cfg.log_dir.clone(), cfg.service_name);

    tracing::info!(
        service = %cfg.service_name,
        version = %cfg.service_version,
        build_sha = %cfg.build_sha,
        deployment_mode = ?cfg.deployment_mode,
        log_dir = %cfg.log_dir.display(),
        otel_enabled = cfg.otel_endpoint.is_some(),
        "observability.init.complete"
    );

    Ok(ObservabilityHandle {
        _appender_guard: Some(guard),
        _sentry_guard: sentry_guard,
        #[cfg(feature = "otel")]
        _otel_provider: otel_provider,
    })
}
