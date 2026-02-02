# Publishing Guide

Step-by-step instructions for publishing this blog post across all platforms.

## Your Assets

```
images/
├── cover-node-22-to-24.png       ← Terminal card "Node 22→24"
├── ci-failure-publish-to-npm.png ← Failed CI run (red ❌ on Publish to npm)
└── ci-success-node-24.png        ← Passing CI run (all green ✅)
```

```
devto.md      ← dev.to version
hashnode.md   ← Hashnode version
medium.md     ← Medium version
reddit.md     ← Reddit version
x-thread.md   ← X/Twitter thread (8 tweets)
```

---

## Step 1: Publish on dev.to (canonical)

This goes first. All other platforms will link back here.

1. Go to https://dev.to/enter
2. Copy-paste the **contents of `devto.md`** into the editor
3. Switch to the markdown editor if you're in rich-text mode (bottom-left toggle)
4. Replace the images:
   - Where you see `![Cover: Node 22→24](./images/cover-node-22-to-24.png)`:
     delete the markdown line, drag `cover-node-22-to-24.png` into that spot.
     dev.to will generate a URL like `https://dev-to-uploads.s3.amazonaws.com/uploads/...`
   - Where you see `![CI run #7...](./images/ci-failure-publish-to-npm.png)`:
     delete the line, drag `ci-failure-publish-to-npm.png`
   - Where you see `![CI run #8...](./images/ci-success-node-24.png)`:
     delete the line, drag `ci-success-node-24.png`
5. For the **cover image** in frontmatter:
   - After dragging the cover into the body, copy its generated URL
   - Paste it into the `cover_image:` line in the frontmatter at the top
   - Remove the comment after it
6. Uncomment and fill in `canonical_url` with the final dev.to post URL (do this after publishing)
7. Update tags if needed (max 4 on dev.to)
8. Preview → Publish
9. **Copy the published URL** — you'll need it for every other platform

---

## Step 2: Cross-post to Hashnode

1. Go to https://hashnode.com/draft → New Story
2. Copy-paste **contents of `hashnode.md`** into the markdown editor
3. Replace images the same way as dev.to:
   - Drag each image file into the spots where you see `![...](...)`
4. Set the cover image:
   - Click the cover image area at the top of the editor
   - Upload `cover-node-22-to-24.png`
5. In frontmatter, set `canonical_url` to your **dev.to URL from Step 1**
   (this tells Google "dev.to is the original" — no SEO penalty for duplicate content)
6. Publish

---

## Step 3: Medium

1. Go to https://medium.com/new-story
2. Click the `+` button → Import a story → doesn't support frontmatter, so do it manually:
   - Type the title: **npm Trusted Publishers Broke My CI and I Blamed OIDC For 3 Days**
   - Type the subtitle: **The 2-line fix nobody told me about**
3. Copy-paste the body from `medium.md` (skip the `# title` and `## subtitle` lines — you already typed those)
4. Replace images:
   - Click on an empty line → press the `+` → Image
   - Upload each image where you see the `![...]` markdown
   - **Add captions** — the italic text below each image in `medium.md` is meant for Medium's image captions. Click the image after inserting and paste the caption text.
5. Set the cover: The first image in your story auto-becomes the preview image.
   Make sure `cover-node-22-to-24.png` is the first image.
6. Before publishing:
   - Click the `...` menu → "More settings"
   - Under "Canonical link", paste your **dev.to URL from Step 1**
7. Optional: Submit to a publication like "Better Programming" or "JavaScript in Plain English"
   for more reach. Click `...` → "Add to publication" before publishing.
8. Publish

---

## Step 4: Reddit

Best subreddits: **r/node**, **r/javascript**, or **r/github**

1. Go to https://www.reddit.com/r/node/submit (or whichever sub)
2. Select "Text" post type
3. Title: `npm trusted publishing silently fails on Node 22 — needs Node 24 (npm >= 11.5.1)`
4. Copy-paste the body from `reddit.md` (skip the title/subreddit lines at the top)
5. Image: optionally drag `ci-failure-publish-to-npm.png` into the editor. One image max.
6. At the bottom where it says `[link to your dev.to/blog post]`, replace with your **dev.to URL from Step 1**
7. Post
8. Optionally cross-post to r/javascript and r/github (use Reddit's crosspost feature)

---

## Step 5: X/Twitter Thread

1. Go to https://x.com/compose/post
2. Open `x-thread.md` and post each numbered block as a **separate tweet in a thread**:

   **Tweet 1:**
   - Copy text from the `1/` block
   - Attach `ci-failure-publish-to-npm.png` (click the image icon)
   - Post

   **Tweet 2:**
   - Reply to tweet 1
   - Copy text from `2/` block
   - No image needed
   - Post

   **Tweet 3:**
   - Reply to tweet 2
   - Copy text from `3/` block
   - No image needed
   - Post

   **Tweet 4:**
   - Reply to tweet 3
   - Copy text from `4/` block
   - No image needed
   - Post

   **Tweet 5:**
   - Reply to tweet 4
   - Copy text from `5/` block
   - Attach `cover-node-22-to-24.png`
   - Post

   **Tweet 6:**
   - Reply to tweet 5
   - Copy text from `6/` block
   - No image needed
   - Post

   **Tweet 7:**
   - Reply to tweet 6
   - Copy text from `7/` block
   - Replace `[link]` with your **dev.to URL from Step 1**
   - Post

   **Tweet 8:**
   - Reply to tweet 7
   - Copy text from `8/` block
   - Attach `ci-success-node-24.png` (the victory screenshot)
   - Post

---

## Quick Reference: What Goes Where

| Image | dev.to | Hashnode | Medium | Reddit | X |
|-------|--------|---------|--------|--------|---|
| `cover-node-22-to-24.png` | Cover + body | Cover + body | First image (becomes preview) | -- | Tweet 5 |
| `ci-failure-publish-to-npm.png` | After "Hit publish" | After "Triple-checked" | After "CI logs showed" | Optional | Tweet 1 |
| `ci-success-node-24.png` | After "The Fix" | After "Pure OIDC" | After "Expired" section | -- | Tweet 8 |

## After Publishing

- [ ] dev.to URL saved: `___________________________`
- [ ] Hashnode canonical_url points to dev.to
- [ ] Medium canonical link points to dev.to
- [ ] Reddit post links to dev.to at bottom
- [ ] X thread tweet 7 links to dev.to
