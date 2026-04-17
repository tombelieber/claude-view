FROM rust:1.85-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the full workspace. The relay depends on transitively-compiled crates
# (observability, core, types, workspace-hack) and the cargo-hakari workspace-hack
# shim — pre-Phase-1 we tried to hand-select individual manifests but that
# broke on every workspace change. Simpler and correct: ship everything.
COPY . .

# Build only the relay binary in release mode. Workspace-hack unifies deps
# so the first build is a one-shot compile of everything; incremental rebuilds
# stay cheap via docker BuildKit layer caching on this COPY.
RUN cargo build -p claude-view-relay --release

# --- Runtime ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/claude-view-relay /usr/local/bin/relay

ENV PORT=8080
EXPOSE 8080

CMD ["relay"]
