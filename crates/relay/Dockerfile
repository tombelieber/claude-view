FROM rust:1.85-slim AS builder

WORKDIR /app

# Copy workspace root
COPY Cargo.toml Cargo.lock ./

# Copy all crate manifests for workspace resolution
COPY crates/relay/Cargo.toml crates/relay/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/search/Cargo.toml crates/search/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml

# Create dummy lib.rs for workspace members (so cargo can resolve deps)
RUN mkdir -p crates/core/src && echo "" > crates/core/src/lib.rs && \
    mkdir -p crates/db/src && echo "" > crates/db/src/lib.rs && \
    mkdir -p crates/search/src && echo "" > crates/search/src/lib.rs && \
    mkdir -p crates/server/src && echo "" > crates/server/src/lib.rs

# Copy relay source
COPY crates/relay/src crates/relay/src

# Build only the relay binary
RUN cargo build -p claude-view-relay --release

# --- Runtime ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/claude-view-relay /usr/local/bin/relay

ENV PORT=8080
EXPOSE 8080

CMD ["relay"]
