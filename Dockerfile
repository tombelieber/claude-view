# ============================================================
# claude-view — Multi-stage Docker build
# Builds the full stack: React frontend + Rust backend + Node.js sidecar
# ============================================================

# ------ Stage 1: Frontend build (Bun + Vite) ------
FROM oven/bun:latest AS frontend

WORKDIR /app

# Copy workspace-level files needed for dependency resolution
COPY package.json bun.lock ./
COPY apps/web/package.json apps/web/
COPY apps/share/package.json apps/share/
COPY apps/landing/package.json apps/landing/
COPY packages/shared/package.json packages/shared/
COPY packages/design-tokens/package.json packages/design-tokens/
COPY packages/mcp/package.json packages/mcp/
COPY packages/plugin/package.json packages/plugin/
COPY sidecar/package.json sidecar/

# Also need infra/share-worker for postinstall script
COPY infra/share-worker/package.json infra/share-worker/

# Install all workspace dependencies (ignore-scripts to avoid postinstall issues, then install sidecar deps separately)
RUN bun install --ignore-scripts

# Copy source files needed for the web build
COPY tsconfig.base.json ./
COPY packages/ packages/
COPY apps/web/ apps/web/

# Build the frontend
RUN cd apps/web && bun run build


# ------ Stage 2: Sidecar build ------
FROM node:22-slim AS sidecar

WORKDIR /app/sidecar

COPY sidecar/package.json sidecar/package-lock.json* sidecar/bun.lock* ./

# Install with npm (more portable in Docker than bun for sidecar)
RUN npm install --production=false 2>/dev/null || npm install

COPY sidecar/ ./

# Build sidecar (tsup bundles to dist/)
RUN npx tsup 2>/dev/null || npm run build


# ------ Stage 3: Rust backend build ------
FROM rust:1.88-slim AS backend

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/search/Cargo.toml crates/search/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml
COPY crates/relay/Cargo.toml crates/relay/Cargo.toml

# Create dummy source files so cargo can resolve and cache dependencies
RUN mkdir -p crates/core/src && echo "pub fn dummy(){}" > crates/core/src/lib.rs && \
    mkdir -p crates/db/src && echo "pub fn dummy(){}" > crates/db/src/lib.rs && \
    mkdir -p crates/search/src && echo "pub fn dummy(){}" > crates/search/src/lib.rs && \
    mkdir -p crates/server/src && echo "fn main(){}" > crates/server/src/main.rs && \
    mkdir -p crates/relay/src && echo "fn main(){}" > crates/relay/src/main.rs

# Disable LTO for faster Docker builds (~3x speedup)
RUN sed -i 's/lto = true/lto = false/' Cargo.toml

# Pre-build dependencies (cached layer — only re-runs if Cargo.toml/lock changes)
RUN cargo build -p claude-view-server --release 2>/dev/null || true

# Copy actual source code
COPY crates/ crates/

# Re-apply LTO override since COPY overwrites Cargo.toml
RUN sed -i 's/lto = true/lto = false/' Cargo.toml

# Touch source files to invalidate the cached dummy build
RUN touch crates/core/src/lib.rs crates/db/src/lib.rs crates/search/src/lib.rs crates/server/src/main.rs

# Build the real binary
RUN cargo build -p claude-view-server --release


# ------ Stage 4: Runtime ------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/* \
    && npm install -g @anthropic-ai/claude-code

WORKDIR /app

# Copy the Rust binary
COPY --from=backend /app/target/release/claude-view /usr/local/bin/claude-view

# Copy the built frontend
COPY --from=frontend /app/apps/web/dist /app/dist

# Copy the sidecar
COPY --from=sidecar /app/sidecar/dist /app/sidecar/dist
COPY --from=sidecar /app/sidecar/package.json /app/sidecar/
COPY --from=sidecar /app/sidecar/node_modules /app/sidecar/node_modules

# Environment configuration for Docker
ENV CLAUDE_VIEW_SKIP_PLATFORM_CHECK=1 \
    CLAUDE_VIEW_BIND_ADDR=0.0.0.0 \
    CLAUDE_VIEW_PORT=47892 \
    CLAUDE_VIEW_DATA_DIR=/data \
    CLAUDE_VIEW_NO_OPEN=1 \
    STATIC_DIR=/app/dist \
    SIDECAR_DIR=/app/sidecar \
    RUST_LOG=warn,claude_view_server=info,claude_view_core=info

# Data volume for DB, search index, config
VOLUME /data

# Claude Code sessions mount point
VOLUME /root/.claude

EXPOSE 47892

# Health check — server responds on /api/health
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:47892/api/health || exit 1

CMD ["claude-view"]
