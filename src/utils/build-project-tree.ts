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
