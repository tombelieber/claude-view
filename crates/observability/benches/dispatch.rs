use criterion::{criterion_group, criterion_main, Criterion};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

fn bench_info_emit(c: &mut Criterion) {
    // Build a subscriber with JSON sink that discards output.
    // Use `set_default` (thread-local) so it doesn't conflict with other tests.
    let subscriber = Registry::default()
        .with(EnvFilter::new("info"))
        .with(fmt::layer().json().with_writer(std::io::sink));
    let _guard = tracing::subscriber::set_default(subscriber);

    c.bench_function("info_event_emit", |b| {
        b.iter(|| {
            tracing::info!(operation = "bench", latency_us = 42, "bench.event");
        });
    });

    c.bench_function("info_span_create_enter_exit", |b| {
        b.iter(|| {
            let span = tracing::info_span!("bench_span", request_id = "abc123");
            let _enter = span.enter();
        });
    });

    c.bench_function("debug_event_filtered_out", |b| {
        b.iter(|| {
            tracing::debug!(x = 1, "filtered.out");
        });
    });
}

criterion_group!(benches, bench_info_emit);
criterion_main!(benches);
