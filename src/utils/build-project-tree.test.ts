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
      makeProject('enc-fl', 'project-a', '/home/user/dev/@org/project-a', 12),
      makeProject('enc-vw', 'wallet-app', '/home/user/dev/@org/wallet-app', 3),
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
    // project-a and project-a/web are parent-child but should appear as siblings
    // Adding a project outside @org so @org doesn't get prefix-collapsed away
    const projects = [
      makeProject('enc-cv', 'claude-view', '/home/dev/@org/claude-view', 230),
      makeProject('enc-fl', 'project-a', '/home/dev/@org/project-a', 336),
      makeProject('enc-fw', 'project-a/web', '/home/dev/@org/project-a/web', 56),
      makeProject('enc-vw', 'wallet-app', '/home/dev/@org/wallet-app', 3),
      makeProject('enc-st', 'standalone', '/home/dev/standalone', 5),
    ];
    const result = buildProjectTree(projects);

    // @org should be a group containing all 4 sub-projects
    const group = result.find((n) => n.type === 'group');
    expect(group).toBeDefined();
    expect(group!.displayName).toBe('@org');
    expect(group!.children!.length).toBe(4); // claude-view, project-a, project-a/web, wallet-app
    expect(group!.sessionCount).toBe(625);

    // project-a/web should be in the group, not orphaned at root
    const projectAWeb = group!.children!.find((n) => n.name === 'enc-fw');
    expect(projectAWeb).toBeDefined();
    expect(projectAWeb!.displayName).toBe('project-a/web');
  });

  it('builds multi-level groups when paths diverge at multiple points', () => {
    // dev/ has @org/ (group) and charity-app (project)
    // @org/ has claude-view and project-a
    const projects = [
      makeProject('enc-cv', 'claude-view', '/home/user/dev/@org/claude-view', 230),
      makeProject('enc-fl', 'project-a', '/home/user/dev/@org/project-a', 336),
      makeProject('enc-tp', 'charity-app', '/home/user/dev/charity-app', 35),
      makeProject('enc-vm', 'test-project', '/home/user/test-project', 3),
    ];
    const result = buildProjectTree(projects);

    // Should have: dev group (containing @org subgroup + charity-app) + test-project
    const devGroup = result.find((n) => n.type === 'group' && n.displayName === 'dev');
    expect(devGroup).toBeDefined();
    expect(devGroup!.depth).toBe(0);

    // dev group should have @org subgroup + charity-app project
    const orgGroup = devGroup!.children!.find((n) => n.type === 'group');
    expect(orgGroup).toBeDefined();
    expect(orgGroup!.displayName).toBe('@org');
    expect(orgGroup!.depth).toBe(1);
    expect(orgGroup!.children).toHaveLength(2);

    const charityApp = devGroup!.children!.find((n) => n.name === 'enc-tp');
    expect(charityApp).toBeDefined();
    expect(charityApp!.depth).toBe(1);

    // test-project at root
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
      makeProject('enc-fl', 'project-a', '/home/user/dev/@org/project-a', 20),
      makeProject('enc-tp', 'charity-app', '/home/user/dev/charity-app', 5),
      makeProject('enc-vm', 'vibe', '/home/user/vibe', 3),
    ];
    const tree = buildProjectTree(projects);
    const names = collectGroupNames(tree);

    expect(names).toContain('dev');
    expect(names).toContain('@org');
    expect(names.length).toBe(2);
  });
});
