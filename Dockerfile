# Package the published version declared by npx-cli, using the same Linux
# artifacts as the normal installer. Build from a published release checkout.
FROM node:24.22.0-trixie-slim@sha256:7f486cabe892bff8fef04c0f7da8dba84a560be3e4e2740b7a86afc57ebec315 AS release
WORKDIR /download
COPY npx-cli/package.json ./package.json
COPY docker/download-release.mjs ./download-release.mjs
RUN node download-release.mjs

FROM node:24.22.0-trixie-slim@sha256:7f486cabe892bff8fef04c0f7da8dba84a560be3e4e2740b7a86afc57ebec315 AS runtime

# Node is required by the bundled sidecar. The pinned CLI runs inside the
# container; host credentials and host project directories are not mounted.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/* \
    && npm install --global --omit=dev @anthropic-ai/claude-code@2.1.296 \
    && rm -rf /root/.npm /usr/local/lib/node_modules/npm /opt/yarn-* \
    && rm -f /usr/local/bin/npm /usr/local/bin/npx /usr/local/bin/yarn /usr/local/bin/yarnpkg \
    && claude --version \
    && mkdir -p /data/home/.claude/projects /data/state /sessions/projects \
    && chown -R node:node /data

COPY --from=release /app /app
RUN /app/claude-view --version

ENV HOME=/data/home \
    CLAUDE_VIEW_DATA_DIR=/data/state \
    CLAUDE_VIEW_CONFIG_DIRS=/sessions \
    CLAUDE_VIEW_BIND_ADDR=0.0.0.0 \
    CLAUDE_VIEW_PORT=47892 \
    CLAUDE_VIEW_SKIP_HOOKS=1 \
    CLAUDE_VIEW_NO_OPEN=1 \
    CLAUDE_VIEW_TELEMETRY=0 \
    DISABLE_AUTOUPDATER=1 \
    STATIC_DIR=/app/dist \
    SIDECAR_DIR=/app/sidecar

USER node
WORKDIR /data/home
EXPOSE 47892
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD ["node", "--eval", "fetch('http://127.0.0.1:47892/api/health', {signal: AbortSignal.timeout(3000)}).then(r => {if (!r.ok) process.exit(1)}).catch(() => process.exit(1))"]
ENTRYPOINT ["/app/claude-view"]
