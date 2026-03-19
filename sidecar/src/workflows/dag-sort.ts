interface Edge {
  source: string
  target: string
}

/**
 * Topological sort using Kahn's algorithm.
 * Phase 1 constraints: rejects cycles and disconnected subgraphs.
 * Self-loops are filtered out before processing.
 */
export function topologicalSort(nodeIds: string[], edges: Edge[]): string[] {
  // Filter self-loops
  const filtered = edges.filter((e) => e.source !== e.target)

  // Single node with no edges is valid
  if (nodeIds.length === 1 && filtered.length === 0) return [...nodeIds]

  // Phase 1: reject disconnected subgraphs (every node must participate in at least one edge)
  if (nodeIds.length > 1) {
    const connected = new Set<string>()
    for (const e of filtered) {
      connected.add(e.source)
      connected.add(e.target)
    }
    for (const id of nodeIds) {
      if (!connected.has(id)) throw new Error(`Disconnected node: ${id}`)
    }
  }

  // Build adjacency list and in-degree map
  const inDegree = new Map<string, number>(nodeIds.map((id) => [id, 0]))
  const adj = new Map<string, string[]>(nodeIds.map((id) => [id, []]))

  for (const { source, target } of filtered) {
    adj.get(source)!.push(target)
    inDegree.set(target, (inDegree.get(target) ?? 0) + 1)
  }

  // Kahn's algorithm: start with zero in-degree nodes
  const queue: string[] = []
  for (const [id, deg] of inDegree) {
    if (deg === 0) queue.push(id)
  }

  const result: string[] = []
  while (queue.length > 0) {
    const node = queue.shift()!
    result.push(node)
    for (const neighbor of adj.get(node) ?? []) {
      const newDeg = inDegree.get(neighbor)! - 1
      inDegree.set(neighbor, newDeg)
      if (newDeg === 0) queue.push(neighbor)
    }
  }

  if (result.length !== nodeIds.length) {
    throw new Error('Cycle detected in workflow graph')
  }

  return result
}
