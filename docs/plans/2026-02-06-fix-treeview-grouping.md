---
status: pending
date: 2026-02-06
---

# Fix TreeView Directory Grouping - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace broken single-level directory grouping in the sidebar tree view with a trie-based algorithm that correctly groups projects by their actual filesystem hierarchy.

**Architecture:** Build a trie (prefix tree) from project `path` values, collapse the common non-branching prefix, then emit groups/projects from where paths diverge. Zero hardcoded paths — purely data-driven.

**Tech Stack:** TypeScript, Vitest, React hooks

**No DB migration needed** — bug is entirely in the frontend `buildProjectTree()` function. The API already returns correct `path` values.

---

### Task 1: Write Failing Tests for Trie-Based Tree Building

**Files:**
- Modify: `src/utils/build-project-tree.test.ts`

**Step 1: Rewrite test suite for new algorithm**

Replace the existing `describe('buildProjectTree')` block. The new tests cover:

1. Empty input
2. Common prefix collapsing (all projects share `/home/user/dev/` → collapsed)
3. Sub-project flattening (`repo/` and `repo/web/` → siblings in same group)
4. Projects encountered during prefix collapse become root-level items
5. Multi-level groups (e.g., `dev/` containing `@org/` subgroup + standalone project)
6. Single project → no group, just root-level project
7. Path `/` (root project) → root-level item
8. `collectGroupNames` helper returns all group names recursively

```typescript
// src/utils/build-project-tree.test.ts

import { describe, it, expect } from 'vitest';
import { buildFlatList, buildProjectTree, collectGroupNames } from './build-project-tree';
import type { ProjectSummary } from '../types/generated/ProjectSummary';

function makeProject(
  name: string,
  displayName: string,
  path: string,
  sessionCount: number
): ProjectSummary {
  return { name, displayName, path, sessionCount, activeCount: 0, lastActivityAt: 0 };
}

describe('buildFlatList', () => {
  it('returns empty array for no projects', () => {
    expect(buildFlatList([])).toEqual([]);
  });

  it('returns flat list sorted alphabetically by displayName', () => {
    const projects = [
      makeProject('z', 'zebra', '/home/user/zebra', 10),
      makeProject('a', 'alpha', '/home/user/alpha', 20),
      makeProject('b', 'beta', '/home/user/beta', 15),
    ];
    const tree = buildFlatList(projects);
    expect(tree).toHaveLength(3);
    expect(tree[0].displayName).toBe('alpha');
    expect(tree[1].displayName).toBe('beta');
    expect(tree[2].displayName).toBe('zebra');
    expect(tree.every((n) => n.type === 'project' && n.depth === 0)).toBe(true);
  });
});

describe('buildProjectTree', () => {
  it('returns empty array for no projects', () => {
    expect(buildProjectTree([])).toEqual([]);
  });

  it('collapses common prefix and groups at the divergence point', () => {
    // All under /home/user/dev/ — common prefix collapses
    // dev/ has 2 children: @org/ (3 projects) and standalone (1 project)
    const projects = [
      makeProject('enc-cv', 'claude-view', '/home/user/dev/@org/claude-view', 47),
      makeProject('enc-fl', 'fluffy', '/home/user/dev/@org/fluffy', 12),
      makeProject('enc-vw', 'vic-wallet', '/home/user/dev/@org/vic-wallet', 3),
      makeProject('enc-st', 'standalone', '/home/user/dev/standalone', 5),
    ];
    const result = buildProjectTree(projects);

    // Should have: @org group + standalone project
    const group = result.find((n) => n.type === 'group');
    expect(group).toBeDefined();
    expect(group!.displayName).toBe('@org');
    expect(group!.children).toHaveLength(3);
    expect(group!.sessionCount).toBe(62); // 47+12+3

    const standalone = result.find((n) => n.name === 'enc-st');
    expect(standalone).toBeDefined();
    expect(standalone!.type).toBe('project');
    expect(standalone!.depth).toBe(0);
  });

  it('collects projects encountered during prefix collapse as root-level items', () => {
    // "/": project at root
    // /Users/Alice: project at home dir
    // /Users/Alice/dev/proj-a: project
    // /Users/Alice/dev/proj-b: project
    const projects = [
      makeProject('root', '/', '/', 1),
      makeProject('alice', 'Alice', '/Users/Alice', 49),
      makeProject('proj-a', 'proj-a', '/Users/Alice/dev/proj-a', 10),
      makeProject('proj-b', 'proj-b', '/Users/Alice/dev/proj-b', 20),
    ];
    const result = buildProjectTree(projects);

    // "/" and "Alice" should be root-level projects (collected during prefix collapse)
    const rootProj = result.find((n) => n.name === 'root');
    expect(rootProj).toBeDefined();
    expect(rootProj!.type).toBe('project');
    expect(rootProj!.depth).toBe(0);

    const aliceProj = result.find((n) => n.name === 'alice');
    expect(aliceProj).toBeDefined();
    expect(aliceProj!.type).toBe('project');
    expect(aliceProj!.depth).toBe(0);

    // proj-a and proj-b should also be present (at root level since dev/ has only 2 children
    // and both are leaves — they share the same parent so they form a group or are at depth 0)
    expect(result.find((n) => n.name === 'proj-a')).toBeDefined();
    expect(result.find((n) => n.name === 'proj-b')).toBeDefined();
  });

  it('handles sub-projects by flattening them as siblings in same group', () => {
    // fluffy and fluffy/web are parent-child but should appear as siblings
    const projects = [
      makeProject('enc-cv', 'claude-view', '/home/dev/@org/claude-view', 230),
      makeProject('enc-fl', 'fluffy', '/home/dev/@org/fluffy', 336),
      makeProject('enc-fw', 'fluffy/web', '/home/dev/@org/fluffy/web', 56),
      makeProject('enc-vw', 'vic-wallet', '/home/dev/@org/vic-wallet', 3),
    ];
    const result = buildProjectTree(projects);

    // All 4 projects should be inside the @org group
    const group = result.find((n) => n.type === 'group');
    expect(group).toBeDefined();
    expect(group!.displayName).toBe('@org');
    expect(group!.children!.length).toBe(4); // claude-view, fluffy, fluffy/web, vic-wallet
    expect(group!.sessionCount).toBe(625);

    // fluffy/web should be in the group, not orphaned at root
    const fluffyWeb = group!.children!.find((n) => n.name === 'enc-fw');
    expect(fluffyWeb).toBeDefined();
    expect(fluffyWeb!.displayName).toBe('fluffy/web');
  });

  it('builds multi-level groups when paths diverge at multiple points', () => {
    // dev/ has @org/ (group) and taipofire-donations (project)
    // @org/ has claude-view and fluffy
    const projects = [
      makeProject('enc-cv', 'claude-view', '/home/user/dev/@org/claude-view', 230),
      makeProject('enc-fl', 'fluffy', '/home/user/dev/@org/fluffy', 336),
      makeProject('enc-tp', 'taipofire', '/home/user/dev/taipofire-donations', 35),
      makeProject('enc-vm', 'vibe-test', '/home/user/vibe-test', 3),
    ];
    const result = buildProjectTree(projects);

    // Should have: dev group (containing @org subgroup + taipofire) + vibe-test
    const devGroup = result.find((n) => n.type === 'group' && n.displayName === 'dev');
    expect(devGroup).toBeDefined();
    expect(devGroup!.depth).toBe(0);

    // dev group should have @org subgroup + taipofire project
    const orgGroup = devGroup!.children!.find((n) => n.type === 'group');
    expect(orgGroup).toBeDefined();
    expect(orgGroup!.displayName).toBe('@org');
    expect(orgGroup!.depth).toBe(1);
    expect(orgGroup!.children).toHaveLength(2);

    const taipofire = devGroup!.children!.find((n) => n.name === 'enc-tp');
    expect(taipofire).toBeDefined();
    expect(taipofire!.depth).toBe(1);

    // vibe-test at root
    const vibeTest = result.find((n) => n.name === 'enc-vm');
    expect(vibeTest).toBeDefined();
    expect(vibeTest!.depth).toBe(0);
  });

  it('returns single project at root level without any group', () => {
    const projects = [
      makeProject('solo', 'solo-project', '/deep/nested/path/solo-project', 42),
    ];
    const result = buildProjectTree(projects);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe('project');
    expect(result[0].name).toBe('solo');
    expect(result[0].depth).toBe(0);
  });

  it('collapses single-child non-project directory chains into one group name', () => {
    // /a/b/c/d/proj1 and /a/b/c/d/proj2 — a/b/c/d is all single-child
    // After prefix collapse to where paths diverge, both are at root
    const projects = [
      makeProject('p1', 'proj1', '/a/b/c/d/proj1', 10),
      makeProject('p2', 'proj2', '/a/b/c/d/proj2', 20),
    ];
    const result = buildProjectTree(projects);

    // Both should be at depth 0 (prefix collapsed all the way to d/ which has 2 children)
    expect(result).toHaveLength(2);
    expect(result.every((n) => n.type === 'project' && n.depth === 0)).toBe(true);
  });

  it('sorts groups before projects, both alphabetically', () => {
    const projects = [
      makeProject('z', 'zebra', '/base/zebra', 1),
      makeProject('gb', 'group-b-proj', '/base/group-b/proj', 2),
      makeProject('ga', 'group-a-proj', '/base/group-a/proj', 3),
      makeProject('a', 'alpha', '/base/alpha', 4),
    ];
    const result = buildProjectTree(projects);

    const firstGroupIdx = result.findIndex((n) => n.type === 'group');
    const firstProjIdx = result.findIndex((n) => n.type === 'project');
    if (firstGroupIdx >= 0 && firstProjIdx >= 0) {
      expect(firstGroupIdx).toBeLessThan(firstProjIdx);
    }
  });

  it('handles project with empty path gracefully', () => {
    const projects: ProjectSummary[] = [
      { name: 'no-path', displayName: 'No Path', path: '', sessionCount: 5, activeCount: 0, lastActivityAt: 0 },
      makeProject('other', 'other', '/home/user/proj', 10),
    ];
    const result = buildProjectTree(projects);
    expect(result.length).toBeGreaterThanOrEqual(2);
    expect(result.find((n) => n.name === 'no-path')).toBeDefined();
  });
});

describe('collectGroupNames', () => {
  it('returns all group names from nested tree', () => {
    const projects = [
      makeProject('enc-cv', 'claude-view', '/home/user/dev/@org/claude-view', 10),
      makeProject('enc-fl', 'fluffy', '/home/user/dev/@org/fluffy', 20),
      makeProject('enc-tp', 'taipofire', '/home/user/dev/taipofire', 5),
      makeProject('enc-vm', 'vibe', '/home/user/vibe', 3),
    ];
    const tree = buildProjectTree(projects);
    const names = collectGroupNames(tree);

    expect(names).toContain('dev');
    expect(names).toContain('@org');
    expect(names.length).toBe(2);
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `bunx vitest run src/utils/build-project-tree.test.ts`
Expected: Multiple FAIL (tests reference new behavior and `collectGroupNames` export that doesn't exist yet)

**Step 3: Commit**

```bash
git add src/utils/build-project-tree.test.ts
git commit -m "test: rewrite buildProjectTree tests for trie-based algorithm"
```

---

### Task 2: Implement Trie-Based `buildProjectTree()`

**Files:**
- Modify: `src/utils/build-project-tree.ts` (rewrite lines 36-140, the `buildProjectTree` function and add helpers)

**Step 1: Replace `buildProjectTree()` with trie-based implementation**

Keep `ProjectTreeNode` interface (lines 5-13) and `buildFlatList` (lines 21-34) unchanged. Replace everything from line 36 to line 141 with:

```typescript
/** Internal trie node for building hierarchical tree from paths. */
interface TrieNode {
  segment: string;
  children: Map<string, TrieNode>;
  project?: ProjectSummary;
}

/**
 * Build a hierarchical tree from flat project list using a trie (prefix tree).
 *
 * Algorithm:
 * 1. Build trie from project paths (split on '/')
 * 2. Collapse common single-child prefix from root (collecting any projects encountered)
 * 3. Emit tree from the first branching point:
 *    - Non-project directory with >1 child → group
 *    - Leaf → project
 *    - Project with sub-projects → project + flattened sub-projects as siblings
 *    - Single-child non-project chains → collapsed into one group name
 */
export function buildProjectTree(projects: ProjectSummary[]): ProjectTreeNode[] {
  if (projects.length === 0) return [];

  // Step 1: Build trie from project paths
  const root: TrieNode = { segment: '', children: new Map() };
  for (const project of projects) {
    const segments = (project.path || '').split('/').filter(Boolean);
    let node = root;
    for (const seg of segments) {
      if (!node.children.has(seg)) {
        node.children.set(seg, { segment: seg, children: new Map() });
      }
      node = node.children.get(seg)!;
    }
    node.project = project;
  }

  // Step 2: Collapse single-child prefix, collecting projects along the way
  const rootProjects: ProjectSummary[] = [];
  let displayRoot = root;
  while (displayRoot.children.size === 1) {
    if (displayRoot.project) {
      rootProjects.push(displayRoot.project);
    }
    displayRoot = [...displayRoot.children.values()][0];
  }
  // If display root is a project with children, collect it as root-level
  if (displayRoot.project && displayRoot.children.size > 0) {
    rootProjects.push(displayRoot.project);
  }

  // Step 3: If display root has no children, it's a leaf
  if (displayRoot.children.size === 0) {
    if (displayRoot.project && !rootProjects.includes(displayRoot.project)) {
      rootProjects.push(displayRoot.project);
    }
    return sortNodes(rootProjects.map((p) => toProjectNode(p, 0)));
  }

  // Step 4: Build tree from display root's children
  const result: ProjectTreeNode[] = rootProjects.map((p) => toProjectNode(p, 0));
  result.push(...emitTrieChildren(displayRoot, 0));
  return sortNodes(result);
}

function toProjectNode(project: ProjectSummary, depth: number): ProjectTreeNode {
  return {
    type: 'project',
    name: project.name,
    displayName: project.displayName,
    path: project.path,
    sessionCount: project.sessionCount,
    depth,
  };
}

function sortNodes(nodes: ProjectTreeNode[]): ProjectTreeNode[] {
  return [...nodes].sort((a, b) => {
    if (a.type !== b.type) return a.type === 'group' ? -1 : 1;
    return a.displayName.localeCompare(b.displayName);
  });
}

function emitTrieChildren(parent: TrieNode, depth: number): ProjectTreeNode[] {
  const result: ProjectTreeNode[] = [];

  for (const child of parent.children.values()) {
    // Collapse single-child non-project chain
    let current = child;
    const nameParts: string[] = [child.segment];
    while (current.children.size === 1 && !current.project) {
      current = [...current.children.values()][0];
      nameParts.push(current.segment);
    }
    const collapsedName = nameParts.join('/');

    if (current.children.size === 0) {
      // Leaf project
      if (current.project) {
        result.push(toProjectNode(current.project, depth));
      }
    } else if (current.project) {
      // Project with sub-projects: emit project + flatten descendants as siblings
      result.push(toProjectNode(current.project, depth));
      result.push(...collectDescendantProjects(current, depth));
    } else {
      // Non-project branching node: create group
      const children = emitTrieChildren(current, depth + 1);
      const sessionCount = children.reduce((sum, n) => sum + n.sessionCount, 0);
      result.push({
        type: 'group',
        name: collapsedName,
        displayName: collapsedName,
        sessionCount,
        depth,
        children: sortNodes(children),
      });
    }
  }

  return sortNodes(result);
}

/** Collect all projects from a node's descendants (not the node itself). */
function collectDescendantProjects(node: TrieNode, depth: number): ProjectTreeNode[] {
  const result: ProjectTreeNode[] = [];
  function recurse(n: TrieNode): void {
    for (const child of n.children.values()) {
      if (child.project) {
        result.push(toProjectNode(child.project, depth));
      }
      recurse(child);
    }
  }
  recurse(node);
  return result;
}

/** Collect all group names from a tree (for auto-expand). */
export function collectGroupNames(nodes: ProjectTreeNode[]): string[] {
  const names: string[] = [];
  for (const node of nodes) {
    if (node.type === 'group') {
      names.push(node.name);
      if (node.children) {
        names.push(...collectGroupNames(node.children));
      }
    }
  }
  return names;
}
```

**Step 2: Run tests to verify they pass**

Run: `bunx vitest run src/utils/build-project-tree.test.ts`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add src/utils/build-project-tree.ts
git commit -m "fix: replace single-level grouping with trie-based tree building"
```

---

### Task 3: Auto-Expand Groups on Tree Mode Switch

**Files:**
- Modify: `src/components/Sidebar.tsx:7` (add import)
- Modify: `src/components/Sidebar.tsx:22-31` (add useEffect)

**Step 1: Add `collectGroupNames` import**

Change line 7 from:
```typescript
import { buildFlatList, buildProjectTree, type ProjectTreeNode } from '../utils/build-project-tree'
```
to:
```typescript
import { buildFlatList, buildProjectTree, collectGroupNames, type ProjectTreeNode } from '../utils/build-project-tree'
```

**Step 2: Add auto-expand effect after the `treeNodes` memo (after line 31)**

Insert after the `treeNodes` useMemo block:

```typescript
  // Auto-expand all groups when switching to tree mode
  const prevViewModeRef = useRef(viewMode)
  useEffect(() => {
    if (viewMode === 'tree' && prevViewModeRef.current !== 'tree') {
      setExpandedGroups(new Set(collectGroupNames(treeNodes)))
    }
    prevViewModeRef.current = viewMode
  }, [viewMode, treeNodes])
```

**Step 3: Verify in browser**

Run: `bun run dev` (if not already running)

1. Open sidebar, switch to tree view
2. All groups should be expanded by default
3. Manually collapse a group → stays collapsed
4. Switch to list view → switch back to tree → all expanded again
5. Sub-projects (like fluffy/web) appear inside correct group alongside parent
6. No orphaned projects at root level
7. Flat list mode unchanged

**Step 4: Commit**

```bash
git add src/components/Sidebar.tsx
git commit -m "feat: auto-expand all groups when switching to tree view"
```

---

### Task 4: Final Verification

**Step 1: Run full test suite**

Run: `bunx vitest run`
Expected: All tests PASS

**Step 2: Visual verification checklist**

- [ ] Tree view: groups have meaningful names (actual dir names like `@org`, `dev`)
- [ ] Tree view: sub-projects appear inside correct group
- [ ] Tree view: no duplicate/orphaned entries
- [ ] Tree view: session counts are correct (groups sum their children)
- [ ] Tree view: expand/collapse works at all depth levels
- [ ] Tree view: clicking a project navigates correctly
- [ ] Tree view: clicking a group expands/collapses it
- [ ] List view: completely unchanged
- [ ] Keyboard navigation still works

**Step 3: Final commit (if any adjustments)**

```bash
git add -A
git commit -m "fix: final adjustments for treeview grouping"
```
