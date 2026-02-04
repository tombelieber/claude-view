// src/utils/build-project-tree.ts

import type { ProjectSummary } from '../types/generated/ProjectSummary';

export interface ProjectTreeNode {
  type: 'project' | 'group';
  name: string;
  displayName: string;
  path?: string; // Only for type: 'project'
  sessionCount: number;
  children?: ProjectTreeNode[];
  depth: number;
}

/**
 * Build a flat list of projects sorted alphabetically.
 *
 * @param projects - Array of project summaries
 * @returns Array of tree nodes (flat list)
 */
export function buildFlatList(projects: ProjectSummary[]): ProjectTreeNode[] {
  if (projects.length === 0) return [];

  return projects
    .map((project) => ({
      type: 'project' as const,
      name: project.name,
      displayName: project.displayName,
      path: project.path,
      sessionCount: project.sessionCount,
      depth: 0,
    }))
    .sort((a, b) => a.displayName.localeCompare(b.displayName));
}

/**
 * Build a tree structure from flat project list based on directory structure.
 *
 * Algorithm:
 * 1. For each project, get the parent directory (last segment before project name)
 * 2. Group projects by their immediate parent directory
 * 3. If multiple projects share a parent, create a group
 * 4. If a parent has only one project, flatten it (no group)
 * 5. Session counts are shown on every row (groups show sum of children)
 *
 * @param projects - Array of project summaries
 * @returns Array of tree nodes (hierarchical structure)
 */
export function buildProjectTree(projects: ProjectSummary[]): ProjectTreeNode[] {
  if (projects.length === 0) return [];

  // Group projects by their immediate parent directory
  const byParent = new Map<string | null, ProjectSummary[]>();

  for (const project of projects) {
    if (!project.path) {
      // No path = add to root with null parent
      const existing = byParent.get(null) || [];
      existing.push(project);
      byParent.set(null, existing);
      continue;
    }

    // Get the immediate parent directory name
    const segments = project.path.split('/').filter(Boolean);
    if (segments.length < 2) {
      // Single segment = no parent, add to root
      const existing = byParent.get(null) || [];
      existing.push(project);
      byParent.set(null, existing);
      continue;
    }

    // Parent is the second-to-last segment
    const parent = segments[segments.length - 2];
    const existing = byParent.get(parent) || [];
    existing.push(project);
    byParent.set(parent, existing);
  }

  const result: ProjectTreeNode[] = [];

  // Process each parent group
  for (const [parent, groupProjects] of byParent.entries()) {
    if (parent === null) {
      // Projects without a parent go directly to root
      for (const project of groupProjects) {
        result.push({
          type: 'project',
          name: project.name,
          displayName: project.displayName,
          path: project.path,
          sessionCount: project.sessionCount,
          depth: 0,
        });
      }
    } else if (groupProjects.length === 1) {
      // Single child = flatten (no group)
      const project = groupProjects[0];
      result.push({
        type: 'project',
        name: project.name,
        displayName: project.displayName,
        path: project.path,
        sessionCount: project.sessionCount,
        depth: 0,
      });
    } else {
      // Multiple children = create a group
      const sessionCount = groupProjects.reduce((sum, p) => sum + p.sessionCount, 0);
      const children = groupProjects
        .map((project) => ({
          type: 'project' as const,
          name: project.name,
          displayName: project.displayName,
          path: project.path,
          sessionCount: project.sessionCount,
          depth: 1,
        }))
        .sort((a, b) => a.displayName.localeCompare(b.displayName));

      result.push({
        type: 'group',
        name: parent,
        displayName: parent,
        sessionCount,
        depth: 0,
        children,
      });
    }
  }

  // Sort: groups first, then projects, both alphabetically
  return result.sort((a, b) => {
    if (a.type !== b.type) {
      return a.type === 'group' ? -1 : 1;
    }
    return a.displayName.localeCompare(b.displayName);
  });
}
