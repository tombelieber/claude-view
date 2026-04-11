// Runtime normalizers for InteractionBlock.data → typed card props.
// Each accepts `unknown` and returns the exact interface the shared card
// component expects, or `null` if required fields are missing/wrong type.
// Per project rule: normalize at boundary — never `as` casts in components.

import type {
  AskQuestion,
  Elicitation,
  PermissionRequest,
  PlanApproval,
} from '../types/sidecar-protocol'

// ─── Helpers ─────────────────────────────────────────────────────

function isNonNullObject(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === 'object' && !Array.isArray(v)
}

function asRecord(v: unknown): Record<string, unknown> | null {
  return isNonNullObject(v) ? v : null
}

// ─── normalizePermissionRequest ──────────────────────────────────

export function normalizePermissionRequest(
  data: unknown,
): PermissionRequest | null {
  if (!isNonNullObject(data)) return null

  const requestId =
    typeof data.requestId === 'string' ? data.requestId : null
  const toolName =
    typeof data.toolName === 'string' ? data.toolName : null

  if (!requestId || !toolName) {
    console.error(
      '[normalizePermissionRequest] Missing required fields',
      data,
    )
    return null
  }

  return {
    type: 'permission_request',
    requestId,
    toolName,
    toolInput: asRecord(data.toolInput) ?? {},
    toolUseID:
      typeof data.toolUseID === 'string' ? data.toolUseID : '',
    suggestions: Array.isArray(data.suggestions)
      ? data.suggestions
      : undefined,
    decisionReason:
      typeof data.decisionReason === 'string'
        ? data.decisionReason
        : undefined,
    blockedPath:
      typeof data.blockedPath === 'string'
        ? data.blockedPath
        : undefined,
    agentID:
      typeof data.agentID === 'string' ? data.agentID : undefined,
    timeoutMs:
      typeof data.timeoutMs === 'number' ? data.timeoutMs : 60000,
  }
}

// ─── normalizeAskQuestion ────────────────────────────────────────

function normalizeOption(
  raw: unknown,
): { label: string; description: string; markdown?: string } | null {
  if (!isNonNullObject(raw)) return null
  const label = typeof raw.label === 'string' ? raw.label : ''
  const description =
    typeof raw.description === 'string' ? raw.description : ''
  const base: { label: string; description: string; markdown?: string } = {
    label,
    description,
  }
  if (typeof raw.markdown === 'string') {
    base.markdown = raw.markdown
  }
  return base
}

function normalizeQuestionItem(raw: unknown): AskQuestion['questions'][number] {
  const d = isNonNullObject(raw) ? raw : ({} as Record<string, unknown>)
  return {
    question: typeof d.question === 'string' ? d.question : '',
    header: typeof d.header === 'string' ? d.header : '',
    options: Array.isArray(d.options)
      ? (d.options
          .map(normalizeOption)
          .filter(Boolean) as AskQuestion['questions'][number]['options'])
      : [],
    multiSelect:
      typeof d.multiSelect === 'boolean' ? d.multiSelect : false,
  }
}

export function normalizeAskQuestion(
  data: unknown,
): AskQuestion | null {
  if (!isNonNullObject(data)) return null

  const requestId =
    typeof data.requestId === 'string' ? data.requestId : null
  if (!requestId) {
    console.error(
      '[normalizeAskQuestion] Missing required fields',
      data,
    )
    return null
  }

  if (!Array.isArray(data.questions)) {
    console.error(
      '[normalizeAskQuestion] questions must be an array',
      data,
    )
    return null
  }

  return {
    type: 'ask_question',
    requestId,
    questions: data.questions.map(normalizeQuestionItem),
  }
}

// ─── normalizePlanApproval ───────────────────────────────────────

export function normalizePlanApproval(
  data: unknown,
): PlanApproval | null {
  if (!isNonNullObject(data)) return null

  const requestId =
    typeof data.requestId === 'string' ? data.requestId : null
  if (!requestId) {
    console.error(
      '[normalizePlanApproval] Missing required fields',
      data,
    )
    return null
  }

  return {
    type: 'plan_approval',
    requestId,
    planData: asRecord(data.planData) ?? {},
  }
}

// ─── normalizeElicitation ────────────────────────────────────────

export function normalizeElicitation(
  data: unknown,
): Elicitation | null {
  if (!isNonNullObject(data)) return null

  const requestId =
    typeof data.requestId === 'string' ? data.requestId : null
  const toolName =
    typeof data.toolName === 'string' ? data.toolName : null
  const prompt =
    typeof data.prompt === 'string' ? data.prompt : null

  if (!requestId || !toolName || !prompt) {
    console.error(
      '[normalizeElicitation] Missing required fields',
      data,
    )
    return null
  }

  return {
    type: 'elicitation',
    requestId,
    toolName,
    toolInput: asRecord(data.toolInput) ?? {},
    prompt,
  }
}
