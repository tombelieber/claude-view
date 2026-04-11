import { describe, expect, it, vi } from 'vitest'
import {
  normalizePermissionRequest,
  normalizeAskQuestion,
  normalizePlanApproval,
  normalizeElicitation,
} from '../interaction-normalizers'

// Suppress console.error in tests — we assert on return value, not log output
vi.spyOn(console, 'error').mockImplementation(() => {})

// ─── normalizePermissionRequest ──────────────────────────────────

describe('normalizePermissionRequest', () => {
  const valid = {
    type: 'permission_request',
    requestId: 'req-1',
    toolName: 'Bash',
    toolInput: { command: 'ls' },
    toolUseID: 'tu-1',
    suggestions: [{ label: 'allow' }],
    decisionReason: 'user policy',
    blockedPath: '/etc',
    agentID: 'agent-1',
    timeoutMs: 30000,
  }

  it('returns typed object for valid data', () => {
    const result = normalizePermissionRequest(valid)
    expect(result).toEqual(valid)
  })

  it('returns null when requestId is missing', () => {
    const { requestId: _, ...rest } = valid
    expect(normalizePermissionRequest(rest)).toBeNull()
  })

  it('returns null when toolName is missing', () => {
    const { toolName: _, ...rest } = valid
    expect(normalizePermissionRequest(rest)).toBeNull()
  })

  it('returns null for non-object inputs', () => {
    expect(normalizePermissionRequest(null)).toBeNull()
    expect(normalizePermissionRequest(undefined)).toBeNull()
    expect(normalizePermissionRequest('string')).toBeNull()
    expect(normalizePermissionRequest(42)).toBeNull()
  })

  it('defaults timeoutMs to 60000 when missing', () => {
    const { timeoutMs: _, ...rest } = valid
    const result = normalizePermissionRequest(rest)
    expect(result).not.toBeNull()
    expect(result!.timeoutMs).toBe(60000)
  })

  it('defaults toolInput to {} when missing', () => {
    const { toolInput: _, ...rest } = valid
    const result = normalizePermissionRequest(rest)
    expect(result).not.toBeNull()
    expect(result!.toolInput).toEqual({})
  })

  it('defaults toolUseID to empty string when missing', () => {
    const { toolUseID: _, ...rest } = valid
    const result = normalizePermissionRequest(rest)
    expect(result).not.toBeNull()
    expect(result!.toolUseID).toBe('')
  })

  it('defaults optional string fields to undefined when missing', () => {
    const minimal = { requestId: 'r', toolName: 'T' }
    const result = normalizePermissionRequest(minimal)
    expect(result).not.toBeNull()
    expect(result!.suggestions).toBeUndefined()
    expect(result!.decisionReason).toBeUndefined()
    expect(result!.blockedPath).toBeUndefined()
    expect(result!.agentID).toBeUndefined()
  })

  it('ignores extra fields', () => {
    const result = normalizePermissionRequest({ ...valid, extraField: 'hi' })
    expect(result).not.toBeNull()
    expect(result!.requestId).toBe('req-1')
  })
})

// ─── normalizeAskQuestion ────────────────────────────────────────

describe('normalizeAskQuestion', () => {
  const valid = {
    type: 'ask_question',
    requestId: 'req-2',
    questions: [
      {
        question: 'Pick one',
        header: 'Choice',
        options: [{ label: 'A', description: 'Option A' }],
        multiSelect: true,
      },
    ],
  }

  it('returns typed object for valid data', () => {
    const result = normalizeAskQuestion(valid)
    expect(result).toEqual(valid)
  })

  it('returns null when requestId is missing', () => {
    const { requestId: _, ...rest } = valid
    expect(normalizeAskQuestion(rest)).toBeNull()
  })

  it('returns null when questions is missing', () => {
    const { questions: _, ...rest } = valid
    expect(normalizeAskQuestion(rest)).toBeNull()
  })

  it('returns null when questions is not an array', () => {
    expect(normalizeAskQuestion({ requestId: 'r', questions: 'bad' })).toBeNull()
  })

  it('returns null for non-object inputs', () => {
    expect(normalizeAskQuestion(null)).toBeNull()
    expect(normalizeAskQuestion(undefined)).toBeNull()
    expect(normalizeAskQuestion('string')).toBeNull()
    expect(normalizeAskQuestion(42)).toBeNull()
  })

  it('accepts empty questions array', () => {
    const result = normalizeAskQuestion({ requestId: 'r', questions: [] })
    expect(result).not.toBeNull()
    expect(result!.questions).toEqual([])
  })

  it('defaults question item fields when partially present', () => {
    const result = normalizeAskQuestion({
      requestId: 'r',
      questions: [{ question: 'What?' }],
    })
    expect(result).not.toBeNull()
    expect(result!.questions[0]).toEqual({
      question: 'What?',
      header: '',
      options: [],
      multiSelect: false,
    })
  })

  it('normalizes option items with defaults', () => {
    const result = normalizeAskQuestion({
      requestId: 'r',
      questions: [
        {
          question: 'Q',
          options: [{ label: 'X' }],
        },
      ],
    })
    expect(result).not.toBeNull()
    expect(result!.questions[0].options[0]).toEqual({
      label: 'X',
      description: '',
    })
  })

  it('preserves markdown on options when present', () => {
    const result = normalizeAskQuestion({
      requestId: 'r',
      questions: [
        {
          question: 'Q',
          options: [{ label: 'X', description: 'd', markdown: '**bold**' }],
        },
      ],
    })
    expect(result!.questions[0].options[0].markdown).toBe('**bold**')
  })

  it('ignores extra fields', () => {
    const result = normalizeAskQuestion({ ...valid, bonus: true })
    expect(result).not.toBeNull()
    expect(result!.requestId).toBe('req-2')
  })
})

// ─── normalizePlanApproval ───────────────────────────────────────

describe('normalizePlanApproval', () => {
  const valid = {
    type: 'plan_approval',
    requestId: 'req-3',
    planData: { steps: ['a', 'b'] },
  }

  it('returns typed object for valid data', () => {
    const result = normalizePlanApproval(valid)
    expect(result).toEqual(valid)
  })

  it('returns null when requestId is missing', () => {
    const { requestId: _, ...rest } = valid
    expect(normalizePlanApproval(rest)).toBeNull()
  })

  it('returns null for non-object inputs', () => {
    expect(normalizePlanApproval(null)).toBeNull()
    expect(normalizePlanApproval(undefined)).toBeNull()
    expect(normalizePlanApproval('string')).toBeNull()
    expect(normalizePlanApproval(42)).toBeNull()
  })

  it('defaults planData to {} when missing', () => {
    const result = normalizePlanApproval({ requestId: 'r' })
    expect(result).not.toBeNull()
    expect(result!.planData).toEqual({})
  })

  it('defaults planData to {} when not an object', () => {
    const result = normalizePlanApproval({ requestId: 'r', planData: 'bad' })
    expect(result).not.toBeNull()
    expect(result!.planData).toEqual({})
  })

  it('ignores extra fields', () => {
    const result = normalizePlanApproval({ ...valid, extra: 1 })
    expect(result).not.toBeNull()
    expect(result!.requestId).toBe('req-3')
  })
})

// ─── normalizeElicitation ────────────────────────────────────────

describe('normalizeElicitation', () => {
  const valid = {
    type: 'elicitation',
    requestId: 'req-4',
    toolName: 'McpTool',
    toolInput: { key: 'val' },
    prompt: 'Enter your API key',
  }

  it('returns typed object for valid data', () => {
    const result = normalizeElicitation(valid)
    expect(result).toEqual(valid)
  })

  it('returns null when requestId is missing', () => {
    const { requestId: _, ...rest } = valid
    expect(normalizeElicitation(rest)).toBeNull()
  })

  it('returns null when toolName is missing', () => {
    const { toolName: _, ...rest } = valid
    expect(normalizeElicitation(rest)).toBeNull()
  })

  it('returns null when prompt is missing', () => {
    const { prompt: _, ...rest } = valid
    expect(normalizeElicitation(rest)).toBeNull()
  })

  it('returns null for non-object inputs', () => {
    expect(normalizeElicitation(null)).toBeNull()
    expect(normalizeElicitation(undefined)).toBeNull()
    expect(normalizeElicitation('string')).toBeNull()
    expect(normalizeElicitation(42)).toBeNull()
  })

  it('defaults toolInput to {} when missing', () => {
    const result = normalizeElicitation({
      requestId: 'r',
      toolName: 'T',
      prompt: 'P',
    })
    expect(result).not.toBeNull()
    expect(result!.toolInput).toEqual({})
  })

  it('defaults toolInput to {} when not an object', () => {
    const result = normalizeElicitation({
      requestId: 'r',
      toolName: 'T',
      prompt: 'P',
      toolInput: 'bad',
    })
    expect(result).not.toBeNull()
    expect(result!.toolInput).toEqual({})
  })

  it('ignores extra fields', () => {
    const result = normalizeElicitation({ ...valid, extra: true })
    expect(result).not.toBeNull()
    expect(result!.requestId).toBe('req-4')
  })
})
