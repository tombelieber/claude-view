// src/utils/build-project-tree.test.ts

import { describe, it, expect } from 'vitest';
import { buildFlatList, buildProjectTree } from './build-project-tree';
import type { ProjectSummary } from '../types/generated/ProjectSummary';

function makeProject(name: string, path: string, sessionCount: number): ProjectSummary {
  return {
    name,
    displayName: name,
    path,
    sessionCount,
    activeCount: 0,
    lastActivityAt: 0,
  };
}

describe('buildFlatList', () => {
  it('returns empty array for no projects', () => {
    expect(buildFlatList([])).toEqual([]);
  });

  it('returns flat list sorted alphabetically', () => {
    const projects = [
      makeProject('zebra', '/home/user/zebra', 10),
      makeProject('alpha', '/home/user/alpha', 20),
      makeProject('beta', '/home/user/beta', 15),
    ];

    const tree = buildFlatList(projects);

    expect(tree).toHaveLength(3);
    expect(tree[0].name).toBe('alpha');
    expect(tree[1].name).toBe('beta');
    expect(tree[2].name).toBe('zebra');
    expect(tree.every((n) => n.type === 'project')).toBe(true);
    expect(tree.every((n) => n.depth === 0)).toBe(true);
  });

  it('preserves session counts', () => {
    const projects = [
      makeProject('project-a', '/home/dev/project-a', 10),
      makeProject('project-b', '/home/dev/project-b', 20),
    ];

    const tree = buildFlatList(projects);

    expect(tree[0].sessionCount).toBe(10);
    expect(tree[1].sessionCount).toBe(20);
  });
});

describe('buildProjectTree', () => {
  it('returns empty array for no projects', () => {
    const tree = buildProjectTree([]);
    expect(tree).toEqual([]);
  });

  it('returns flat list for projects without shared parent', () => {
    const projects = [
      makeProject('proj-a', '/home/user/proj-a', 5),
      makeProject('proj-b', '/opt/proj-b', 3),
      makeProject('proj-c', '/var/proj-c', 7),
    ];

    const result = buildProjectTree(projects);

    expect(result).toHaveLength(3);
    expect(result.every((node) => node.type === 'project')).toBe(true);
    expect(result.every((node) => node.depth === 0)).toBe(true);
  });

  it('groups projects with shared parent directory', () => {
    const projects = [
      makeProject('claude-view', '/Users/dev/@myorg/claude-view', 47),
      makeProject('project-a', '/Users/dev/@myorg/project-a', 12),
      makeProject('standalone', '/Users/dev/standalone', 5),
    ];

    const result = buildProjectTree(projects);

    // Should have: @myorg group + standalone project
    expect(result).toHaveLength(2);

    // First should be group (groups sort before projects)
    const group = result.find((n) => n.type === 'group');
    expect(group).toBeDefined();
    expect(group!.name).toBe('@myorg');
    expect(group!.sessionCount).toBe(59); // 47 + 12
    expect(group!.children).toHaveLength(2);
    expect(group!.children![0].name).toBe('claude-view');
    expect(group!.children![1].name).toBe('project-a');

    // Second should be standalone project
    const standalone = result.find((n) => n.type === 'project' && n.name === 'standalone');
    expect(standalone).toBeDefined();
    expect(standalone!.depth).toBe(0);
  });

  it('flattens single-child groups', () => {
    const projects = [
      makeProject('only-child', '/Users/dev/parent/only-child', 10),
      makeProject('other', '/opt/other', 5),
    ];

    const result = buildProjectTree(projects);

    // Should flatten parent directory since it only has one child
    expect(result).toHaveLength(2);
    expect(result.every((n) => n.type === 'project')).toBe(true);
    expect(result.find((n) => n.name === 'only-child')).toBeDefined();
    expect(result.find((n) => n.name === 'other')).toBeDefined();
  });

  it('correctly sums session counts for groups', () => {
    const projects = [
      makeProject('proj1', '/workspace/team/proj1', 10),
      makeProject('proj2', '/workspace/team/proj2', 20),
      makeProject('proj3', '/workspace/team/proj3', 15),
    ];

    const result = buildProjectTree(projects);

    // Should create a group for "team"
    const group = result.find((n) => n.type === 'group');
    expect(group).toBeDefined();
    expect(group!.sessionCount).toBe(45); // 10 + 20 + 15
  });

  it('handles projects without paths', () => {
    const projects: ProjectSummary[] = [
      { name: 'no-path', displayName: 'No Path', path: undefined, sessionCount: 5, activeCount: 0, lastActivityAt: 0 },
      makeProject('with-path', '/home/user/proj', 10),
    ];

    const result = buildProjectTree(projects);

    expect(result).toHaveLength(2);
    expect(result.find((n) => n.name === 'no-path')).toBeDefined();
    expect(result.find((n) => n.name === 'with-path')).toBeDefined();
  });

  it('sorts groups before projects, both alphabetically', () => {
    const projects = [
      makeProject('zebra', '/home/zebra', 1),
      makeProject('group-b-proj', '/shared/group-b/proj', 2),
      makeProject('group-a-proj', '/shared/group-a/proj', 3),
      makeProject('alpha', '/home/alpha', 4),
    ];

    const result = buildProjectTree(projects);

    // Find groups and projects
    const groups = result.filter((n) => n.type === 'group');
    const projects_flat = result.filter((n) => n.type === 'project');

    expect(groups.length).toBeGreaterThan(0);
    expect(projects_flat.length).toBeGreaterThan(0);

    // Groups should come before projects in the array
    const firstGroupIndex = result.findIndex((n) => n.type === 'group');
    const firstProjectIndex = result.findIndex((n) => n.type === 'project');
    expect(firstGroupIndex).toBeLessThan(firstProjectIndex);
  });

  it('handles complex real-world directory structure', () => {
    const projects = [
      makeProject('claude-view', '/Users/dev/@myorg/claude-view', 47),
      makeProject('project-a', '/Users/dev/@myorg/project-a', 12),
      makeProject('dotfiles', '/Users/dev/personal/dotfiles', 3),
      makeProject('blog', '/Users/dev/personal/blog', 8),
      makeProject('standalone', '/Users/dev/standalone', 5),
    ];

    const result = buildProjectTree(projects);

    // Should have: @myorg group, personal group, standalone project
    expect(result.length).toBeGreaterThanOrEqual(3);

    const myprojectGroup = result.find((n) => n.name === '@myorg');
    expect(myprojectGroup).toBeDefined();
    expect(myprojectGroup!.type).toBe('group');
    expect(myprojectGroup!.sessionCount).toBe(59);
    expect(myprojectGroup!.children).toHaveLength(2);

    const personalGroup = result.find((n) => n.name === 'personal');
    expect(personalGroup).toBeDefined();
    expect(personalGroup!.type).toBe('group');
    expect(personalGroup!.sessionCount).toBe(11);
    expect(personalGroup!.children).toHaveLength(2);

    const standaloneProject = result.find((n) => n.name === 'standalone');
    expect(standaloneProject).toBeDefined();
    expect(standaloneProject!.type).toBe('project');
  });

  it('sets correct depth for nested children', () => {
    const projects = [
      makeProject('proj1', '/workspace/team/proj1', 10),
      makeProject('proj2', '/workspace/team/proj2', 20),
    ];

    const result = buildProjectTree(projects);

    const group = result.find((n) => n.type === 'group');
    expect(group).toBeDefined();
    expect(group!.depth).toBe(0);
    expect(group!.children![0].depth).toBe(1);
    expect(group!.children![1].depth).toBe(1);
  });
});
