# Run the dashboard with Docker

Docker Compose runs the Linux release on macOS, Linux, or Docker Desktop with
Linux containers on Windows. Use a checkout of a **published release**: the image
downloads the version in `npx-cli/package.json`, verifies its SHA-256 checksum,
and includes its frontend and sidecar. This packages the published application;
it does not compile changes from an unreleased source branch.

## Start

On macOS or Linux, with Claude sessions in `~/.claude/projects`:

```sh
docker compose up --build -d
```

For another location, set `CLAUDE_PROJECTS_DIR` to the **projects directory**:

```sh
CLAUDE_PROJECTS_DIR=/path/to/.claude/projects docker compose up --build -d
```

On Windows PowerShell with Docker Desktop in Linux-container mode:

```powershell
$env:CLAUDE_PROJECTS_DIR = "$env:USERPROFILE/.claude/projects"
docker compose up --build -d
```

Open <http://127.0.0.1:47892>. Set `CLAUDE_VIEW_PORT` to change the host port.
Missing source directories fail instead of creating an empty host directory.

```sh
docker compose logs -f claude-view
docker compose down
```

The project-scoped named volume keeps the index, dashboard settings and
container-owned sessions across restarts and `down`. `docker compose down -v`
**deletes that container data**; it does not delete host transcripts.

## What is shared

Only the host `projects` directory is mounted, **read-only**, as the `/sessions`
profile. History, search, analytics and newly written transcript files can be
read from it. The host's credentials, settings, hooks and project source code
are not mounted. Automatic host hook registration is disabled.

Archiving/deleting host transcripts and controlling host terminals or host
processes are unavailable in this configuration. Host project paths (including
Windows paths) are metadata; they do not expose that project to the container.
Live hook delivery requires a separately configured host hook; the container
does not install one. Existing host settings are left untouched.

The pinned Claude CLI and sidecar run **inside the container**. New agent
sessions need separately configured provider credentials and container-visible
working directories; logging in to Claude on the host does not log in the
container. The default Compose setup supplies no model credentials.

## Boundaries

The service is published on host **127.0.0.1 only**, with a non-root user, a
read-only image filesystem, dropped Linux capabilities and no privilege
escalation. Writable app state stays in its named volume and temporary files in
`/tmp`. Telemetry is off by default for this container.

This dashboard has no network login. Keep the localhost port mapping; do not
publish it directly to a LAN or the internet. Docker administrators can still
access containers they control. Do not mount the Docker socket, host root,
credentials, or a writable host `.claude` directory.

The image supports Linux arm64 and amd64. Windows uses the Linux image through
Docker Desktop; this does not provide a native Windows binary.

The image is not claimed to be free of all advisories. At qualification,
Docker Scout still reports Debian zlib advisory
[CVE-2026-85091](https://security-tracker.debian.org/tracker/CVE-2026-85091).
Debian's package status and the affected-version range remain disputed in the
[upstream discussion](https://github.com/madler/zlib/issues/1310). The app binary
does not import the affected gzip writer functions; Node uses its own bundled
zlib. This is a reachability assessment, not a vendor-approved dismissal of the
advisory. The finding is retained, not suppressed.

## Local verification

```sh
node --test docker/download-release.test.mjs
node test/docker-smoke.mjs
```

The smoke test uses synthetic transcripts, an available localhost port and a
unique Compose project. It verifies discovery, messages, export, the served
frontend, sidecar startup, denied host writes, non-root execution and state
surviving container recreation, then removes only its own containers and volume.
The local pre-push hook runs these checks when Docker packaging changes.

Docker support was adapted from the focused packaging changes by
[@hoklims in PR #24](https://github.com/tombelieber/claude-view/pull/24), without
merging that branch's unrelated history or platform changes.
