---
status: pending
date: 2026-02-03
---

# README Media Preparation Guide

## 1. Screenshot (`docs/screenshot.png`)

### Setup
- Run the app: `bun dev` or `npx claude-view`
- Browser window: **1280x800** or **1440x900**
- Light mode, bookmarks bar hidden, clean tab, no extensions visible

### What to Capture
One hero screenshot showing the most visually rich view — a session conversation with:
- Visible project name / session title
- At least one syntax-highlighted code block
- Some tool usage or skill badges visible
- Enough content to show the app isn't empty

### How to Capture
- `Cmd+Shift+4` → drag to select browser content area (crop out browser chrome / URL bar)
- Save as **PNG** to `docs/screenshot.png`
- If over 500KB: `pngquant docs/screenshot.png --output docs/screenshot.png --force`

### Checklist
- [ ] App running with real session data
- [ ] Screenshot captured and saved to `docs/screenshot.png`
- [ ] File size under 500KB

---

## 2. YouTube Demo Video

### Setup
- Same browser setup as screenshot
- Recording tool: `Cmd+Shift+5` → "Record Selected Portion" or [Kap](https://getkap.co/)
- Have a few projects with multiple sessions so the UI looks populated

### Recording Flow (~20-30 seconds)

| Step | Action | Why |
|------|--------|-----|
| 1 | Land on the project list page | Shows the overview / entry point |
| 2 | Click into a project with several sessions | Shows project → session hierarchy |
| 3 | Hover/scroll through session cards | Shows rich previews (tools, skills) |
| 4 | Click into one session | Shows full conversation with code highlighting |
| 5 | Scroll through the conversation | Shows markdown rendering, code blocks |
| 6 | Hit `Cmd+K` and type a search query | The "wow" moment |
| 7 | (Optional) Export a session | Shows export if quick |

### Recording Tips
- Move cursor deliberately — no frantic mouse movements
- Pause 1-2 seconds on each screen before acting
- Don't click too fast
- Pick a session with interesting content (code blocks, tool usage)

### Post-Recording
1. Trim dead time at start/end
2. Export as MP4 (H.264)
3. Upload to YouTube
4. Copy the video ID (the `v=` part from the URL, e.g. `dQw4w9WgXcQ`)

### Checklist
- [ ] Video recorded and trimmed
- [ ] Uploaded to YouTube
- [ ] Video ID copied

---

## 3. Plugging Into READMEs

Once you have both, tell Claude:

> Here's my YouTube video ID: `XXXXX` and `docs/screenshot.png` is ready.

All 3 READMEs already have `YOUTUBE_VIDEO_ID` placeholders and a commented-out screenshot block ready to be wired up.
