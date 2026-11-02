# Periodic Git sync workload

Periodic sync uses an indexer-owned cache. Explicit `/api/sync/git` requests
always refresh. The interval setting is unchanged.

The cache groups project aliases and subdirectories by canonical **per-worktree**
Git directory. Linked worktrees retain separate HEADs and scans, following the
[Git repository layout](https://git-scm.com/docs/gitrepository-layout). Matching still
uses each session's original project path.

A changed HEAD, symbolic ref, packed refs, shallow boundary, local configuration,
index revision, session time window or session membership invalidates the group.
Changed groups run the existing bounded history scan (latest 100 commits), rather
than an author-timestamp watermark that could miss resets or rewritten history.
Unrecognized/unreadable metadata disables skipping. Replacement refs and reftable
repositories take this conservative path. Deleted inputs are evicted; failed
correlation groups retry without invalidating healthy groups. Cache publication
happens after successful DB work and a stable before/after repository fingerprint.

There is also a 600-second safety refresh for out-of-band object/global-config
edits not represented in the fingerprint. This is a maximum cache age, not a
replacement for immediate invalidation of ordinary ref/session changes.

Git correlation subprocesses resolve the executable to an absolute path in
process, avoiding repeated OS PATH spawn attempts. Missing results are not cached;
a removed/non-executable cached binary triggers resolution again. Commands clear
repository-routing environment variables inherited from Git hooks and kill their
children if a timeout drops the command future.

## Measured subprocess counts

macOS fixture comparison against unmodified `d3e4ad89` (v0.46.2), with two regular
repositories, one initial commit each, and session windows outside commit times.
One repository receives a second commit for the changed case. This isolates scan
work from diff-stat extraction. The test process PATH has 16 nonexistent entries
before `/usr/bin`; no user repositories or session files are involved.

A local `posix_spawn` interposer counted return codes between the workload test's
`git-workload begin/end` markers, excluding fixture setup and mutation commands.

| Tick | Before: successful / ENOENT | After: successful / ENOENT | Repositories scanned before / after |
| --- | ---: | ---: | ---: |
| Idle after warmup | 4 / 64 | 0 / 0 | 2 / 0 |
| One repository changed | 4 / 64 | 2 / 0 | 2 / 1 |

Independent minimal probe: three `Command::new("git")` calls produced 3 successful
spawns and 48 ENOENT attempts; absolute executable paths produced 3 successes and
zero ENOENT attempts. Failed attempts are not successful child processes.

Re-run the behavioral workload and invalidation regressions locally:

```sh
./scripts/cq test -p claude-view-db --lib git_correlation
./scripts/cq test -p claude-view-server --test git_sync_background_test
```

The server integration test exercises the production background helper, status
HTTP endpoint and explicit sync SSE on an ephemeral loopback port, then closes
the listener. It does not bootstrap the installed application or register hooks.

These counts describe Git correlation work, not whole-machine CPU, temperature,
security-review counts, or total startup/indexing/snapshot workload. Safety
refreshes and unsupported metadata deliberately perform scans. Diff-stat work
still occurs for newly linked commits; existing stored-link retention semantics
are unchanged. Status metadata remains per-last-run: an idle tick reports zero
commits scanned, while previously linked commits remain in the database.
