---
status: done
date: 2026-02-02
---

# Security Audit — Critical Fixes

> All findings are README/documentation issues that directly mislead users.

**Source:** Full security + README audit performed 2026-02-02 across 5 parallel scans (secrets, dependencies, unsafe code, file exposure, README accuracy).

---

## Task 1: Add or remove the broken screenshot

**File:** `README.md`, `README.zh-TW.md`, `README.zh-CN.md` (line 4 in each)

**Problem:** All three READMEs reference `./docs/screenshot.png` but the file does not exist. The repo landing page shows a broken image — the first thing any visitor sees.

**Fix (choose one):**
- A) Capture a real screenshot of the app and save to `docs/screenshot.png`
- B) Remove the `<img>` tag from all three README files until a screenshot is ready

---

## Task 2: Remove or label Homebrew install as "Coming Soon"

**Files:** `README.md:72`, `README.zh-TW.md:72`, `README.zh-CN.md:72`

**Problem:** All three READMEs list `brew install claude-view` as an install option. No Homebrew formula, tap, or CI publish step exists. Users who try this get an error.

**Fix:** In all three README files, either:
- A) Remove the Homebrew section entirely
- B) Change it to "Coming Soon" with a note that it's not yet available

---

## Task 3: Sync Chinese README platform tables with English

**Files:** `README.zh-TW.md:102-109`, `README.zh-CN.md:102-109`

**Problem:** The English README correctly shows Linux (x64) and Windows (x64) as "Available". The zh-TW and zh-CN READMEs still show all non-macOS platforms as "Coming" (listing Linux as v2.1, Windows as v2.2). The CI workflow already builds `linux-x64` and `win32-x64` binaries. Chinese-speaking users think the tool is macOS-only.

**Fix:** Update both Chinese README platform roadmap tables to match the English version:
- Linux (x64): Available ✅
- Windows (x64): Available ✅
- Linux (ARM64): Coming
- Windows (ARM64): Coming

Also update the platform badge in all three READMEs from "macOS" to "macOS | Linux | Windows".

---

## Task 4: Remove manual npm publish instructions

**File:** `README.md:150-153`

**Problem:** The Releasing section tells users to run `cd npx-cli && npm publish` manually after CI finishes. The CI workflow (`.github/workflows/release.yml:130-153`) has a `publish-npm` job that auto-publishes via OIDC trusted publishing. The release script (`scripts/release.sh:24`) also confirms this is automated. Manual publish would cause version conflicts.

**Fix:** Remove the `cd npx-cli && npm publish` line. Replace with a note that npm publishing is handled automatically by CI after the GitHub Release is created.

---

## Verification

After all fixes:
- [ ] `docs/screenshot.png` exists OR `<img>` tags removed from all 3 READMEs
- [ ] `brew install claude-view` removed or marked "Coming Soon" in all 3 READMEs
- [ ] Platform tables in zh-TW and zh-CN match English (Linux x64 + Win x64 = Available)
- [ ] Platform badge updated in all 3 READMEs
- [ ] No manual `npm publish` command in README.md Releasing section
- [ ] All three README files render correctly on GitHub (no broken images, no dead instructions)
