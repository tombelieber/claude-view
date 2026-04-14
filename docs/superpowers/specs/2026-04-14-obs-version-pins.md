# Observability Version Pins & JSON Schema

De-risk POC results from Task 0 (2026-04-14).

## Pinned Dependency Versions

| Crate | Pinned Version | Cargo.toml Spec | Notes |
|---|---|---|---|
| tracing | 0.1.44 | `"0.1"` | Already in workspace |
| tracing-core | 0.1.36 | (transitive) | Pulled by tracing |
| tracing-subscriber | 0.3.23 | `"0.3"` | Workspace has `env-filter`; need `json`, `registry`, `fmt` features added |
| tracing-appender | 0.2.4 | `"0.2"` | Rolling file writer with non-blocking support |
| tracing-log | 0.2.0 | (transitive) | Bridges log crate to tracing |
| tracing-opentelemetry | 0.32.1 | `"0.32"` | OTel bridge layer for tracing |
| opentelemetry | 0.31.0 | `"0.31"` | Core OTel API |
| opentelemetry_sdk | 0.31.0 | `"0.31"` | OTel SDK with `rt-tokio` feature |
| opentelemetry-otlp | 0.31.1 | `"0.31"` | OTLP exporter with `grpc-tonic` feature |
| tonic | 0.14.5 | (transitive) | gRPC transport for OTLP |
| prost | 0.14.3 | (transitive) | Protobuf codec for OTLP |
| ulid | 1.2.1 | `"1"` | Request ID generation |
| sha2 | 0.10.9 | `"0.10"` | Host hash computation |
| serde_json | 1.0.149 | `"1"` | Already in workspace |
| hex | 0.4.3 | `"0.4"` | Already in workspace |
| tokio | 1.51.1 | `"1.43"` | Already in workspace (for OTel rt-tokio) |
| serde | 1.0.228 | `"1"` | Already in workspace |

## OTel Compatibility Status

**COMPATIBLE** -- all four OTel crates resolve cleanly with the existing workspace.

The version set `tracing-opentelemetry 0.32` + `opentelemetry{,_sdk,-otlp} 0.31` compiles
without conflict against the workspace's existing tokio 1.43, hyper 1, reqwest 0.12,
and tonic 0.14 dependencies. No feature-unification issues observed.

## tracing-subscriber fmt::json() Field Schema

Captured from POC run with these layer options:
- `.json()` + `.with_target(true)` + `.with_thread_ids(true)` + `.with_thread_names(true)`
- `.with_file(true)` + `.with_line_number(true)` + `.with_span_list(true)` + `.with_current_span(true)`

### Top-Level Fields (10 total)

| Field | Type | Description | Example |
|---|---|---|---|
| `timestamp` | string (RFC 3339) | Event timestamp, microsecond precision | `"2026-04-14T16:17:12.199636Z"` |
| `level` | string | Log level, uppercase | `"INFO"`, `"WARN"` |
| `target` | string | Rust module path | `"observability_poc"` |
| `filename` | string | Source file path (relative to workspace) | `"crates/observability-poc/src/main.rs"` |
| `line_number` | integer | Source line number | `77` |
| `fields` | object | Event-level key-value pairs | `{"component":"poc","action":"validate_deps","message":"..."}` |
| `span` | object | Current (innermost) span's fields + name | `{"name":"poc_request","request_id":"01KP..."}` |
| `spans` | array of objects | Full span stack (outermost first) | `[{"name":"poc_request","request_id":"01KP..."}]` |
| `threadId` | string | Thread ID | `"ThreadId(1)"` |
| `threadName` | string | Thread name | `"main"` |

### fields Object Keys

User-defined event fields are flattened into this object. The `message` key holds the
format string passed to `info!()` / `warn!()` / etc.

### span / spans Object Keys

Each span object contains:
- `name` -- the span's static name (first argument to `info_span!()`)
- All key-value pairs recorded on that span (e.g. `request_id`, `host_hash`, `otel.name`)

### Sample JSON Line (pretty-printed)

```json
{
  "fields": {
    "action": "validate_deps",
    "component": "poc",
    "message": "tracing-subscriber JSON layer initialized successfully"
  },
  "filename": "crates/observability-poc/src/main.rs",
  "level": "INFO",
  "line_number": 77,
  "span": {
    "host_hash": "93be81bae7dc3ab8",
    "name": "poc_request",
    "otel.name": "POC test span",
    "request_id": "01KP6CGN070NGFGR96C6M30C7G"
  },
  "spans": [
    {
      "host_hash": "93be81bae7dc3ab8",
      "name": "poc_request",
      "otel.name": "POC test span",
      "request_id": "01KP6CGN070NGFGR96C6M30C7G"
    }
  ],
  "target": "observability_poc",
  "threadId": "ThreadId(1)",
  "threadName": "main",
  "timestamp": "2026-04-14T16:17:12.199636Z"
}
```

### Design Implications for crates/observability

1. **Custom fields go in `fields` object** -- not at top level. The observability crate's
   `init()` can add custom fields (service_name, host_hash, build_sha) via span context,
   and they will appear in `span`/`spans` objects.

2. **`span` vs `spans`** -- `span` is the innermost (current) span. `spans` is the full
   stack. For single-layer request tracing, they're equivalent. For nested spans
   (request > handler > db_query), `spans` gives the full context chain.

3. **`threadId` format** -- `"ThreadId(N)"` is not a bare integer. If we need to parse
   it downstream, we'll need to strip the wrapper. Consider using `with_thread_ids(false)`
   and adding a custom `tid` field instead, or just accept the format.

4. **`filename` is relative** -- relative to the cargo workspace root, which is what we want
   for log analysis. No need to strip absolute paths.

5. **`message` lives inside `fields`** -- not at top level. Any log parsing that needs the
   human-readable message must look at `fields.message`.
