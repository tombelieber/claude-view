const MAX_INDENT = 5

export interface ThreadInfo {
  indent: number
  isChild: boolean
  parentUuid: string | undefined
}

interface HasUuids {
  uuid?: string | null
  parent_uuid?: string | null
}

export function buildThreadMap(messages: HasUuids[]): Map<string, ThreadInfo> {
  const map = new Map<string, ThreadInfo>()

  const uuidSet = new Set<string>()
  const parentOf = new Map<string, string>()

  for (const msg of messages) {
    if (!msg.uuid) continue
    uuidSet.add(msg.uuid)
    if (msg.parent_uuid && msg.parent_uuid !== msg.uuid) {
      parentOf.set(msg.uuid, msg.parent_uuid)
    }
  }

  const indentCache = new Map<string, number>()
  const inCycle = new Set<string>()

  // First pass: detect all nodes involved in cycles
  function detectCycles(uuid: string, path: Set<string>): void {
    if (inCycle.has(uuid) || !uuidSet.has(uuid)) return
    if (path.has(uuid)) {
      // Mark all nodes in the cycle
      for (const node of path) inCycle.add(node)
      return
    }
    const parent = parentOf.get(uuid)
    if (!parent || !uuidSet.has(parent)) return
    path.add(uuid)
    detectCycles(parent, path)
  }

  for (const uuid of uuidSet) {
    detectCycles(uuid, new Set())
  }

  function computeIndent(uuid: string): number {
    if (indentCache.has(uuid)) return indentCache.get(uuid)!
    if (inCycle.has(uuid)) {
      indentCache.set(uuid, 0)
      return 0
    }

    const parent = parentOf.get(uuid)
    if (!parent || !uuidSet.has(parent)) {
      indentCache.set(uuid, 0)
      return 0
    }

    const parentIndent = computeIndent(parent)
    const indent = Math.min(parentIndent + 1, MAX_INDENT)
    indentCache.set(uuid, indent)
    return indent
  }

  for (const msg of messages) {
    if (!msg.uuid) continue

    const parent = parentOf.get(msg.uuid)
    const hasValidParent = !!parent && uuidSet.has(parent)
    const indent = computeIndent(msg.uuid)

    map.set(msg.uuid, {
      indent,
      isChild: hasValidParent && indent > 0,
      parentUuid: hasValidParent ? parent : undefined,
    })
  }

  return map
}
