// sidecar/src/permission-handler.ts
import type { PermissionResult, PermissionUpdate } from '@anthropic-ai/claude-agent-sdk'
import type {
  AskQuestion,
  Elicitation,
  PermissionRequest,
  PlanApproval,
  ServerEvent,
} from './protocol.js'

interface CanUseToolOptions {
  signal: AbortSignal
  suggestions?: PermissionUpdate[]
  blockedPath?: string
  decisionReason?: string
  toolUseID: string
  agentID?: string
}

interface PendingPermission {
  resolve: (result: PermissionResult) => void
  timer: ReturnType<typeof setTimeout> | null
  originalInput: Record<string, unknown>
}

interface PendingQuestion {
  resolve: (result: PermissionResult) => void
}

interface PendingPlan {
  resolve: (result: PermissionResult) => void
  originalInput: Record<string, unknown>
}

interface PendingElicitation {
  resolve: (result: PermissionResult) => void
}

export class PermissionHandler {
  private permissions = new Map<string, PendingPermission>()
  private questions = new Map<string, PendingQuestion>()
  private plans = new Map<string, PendingPlan>()
  private elicitations = new Map<string, PendingElicitation>()

  async handleCanUseTool(
    toolName: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
    config?: { timeoutMs?: number },
  ): Promise<PermissionResult> {
    const requestId = crypto.randomUUID()
    const timeoutMs = config?.timeoutMs ?? 60_000

    if (toolName === 'AskUserQuestion') {
      return this.handleQuestion(requestId, input, options, emit)
    }
    if (toolName === 'ExitPlanMode') {
      return this.handlePlan(requestId, input, options, emit)
    }
    // MCP elicitation — detect by shape (has prompt field, not a standard tool)
    if (
      input.prompt &&
      typeof input.prompt === 'string' &&
      !['Read', 'Edit', 'Write', 'Bash', 'Grep', 'Glob'].includes(toolName)
    ) {
      return this.handleElicitation(requestId, toolName, input, options, emit)
    }

    return this.handlePermission(requestId, toolName, input, options, emit, timeoutMs)
  }

  private handleQuestion(
    requestId: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      this.questions.set(requestId, {
        resolve: (result) => resolve(result),
      })

      options.signal.addEventListener(
        'abort',
        () => {
          if (this.questions.has(requestId)) {
            this.questions.delete(requestId)
            resolve({ behavior: 'deny', message: 'Question aborted' })
          }
        },
        { once: true },
      )

      emit({
        type: 'ask_question',
        requestId,
        questions: input.questions as AskQuestion['questions'],
      } satisfies AskQuestion)
    })
  }

  private handlePlan(
    requestId: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      this.plans.set(requestId, {
        resolve: (result) => resolve(result),
        originalInput: input,
      })

      options.signal.addEventListener(
        'abort',
        () => {
          if (this.plans.has(requestId)) {
            this.plans.delete(requestId)
            resolve({ behavior: 'deny', message: 'Plan approval aborted' })
          }
        },
        { once: true },
      )

      emit({
        type: 'plan_approval',
        requestId,
        planData: input,
      } satisfies PlanApproval)
    })
  }

  private handleElicitation(
    requestId: string,
    toolName: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      this.elicitations.set(requestId, {
        resolve: (result) => resolve(result),
      })

      options.signal.addEventListener(
        'abort',
        () => {
          if (this.elicitations.has(requestId)) {
            this.elicitations.delete(requestId)
            resolve({ behavior: 'deny', message: 'Elicitation aborted' })
          }
        },
        { once: true },
      )

      emit({
        type: 'elicitation',
        requestId,
        toolName,
        toolInput: input,
        prompt: (input.prompt as string) ?? '',
      } satisfies Elicitation)
    })
  }

  private handlePermission(
    requestId: string,
    toolName: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
    timeoutMs: number,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        if (this.permissions.has(requestId)) {
          this.permissions.delete(requestId)
          resolve({ behavior: 'deny', message: `Permission for ${toolName} timed out` })
        }
      }, timeoutMs)

      this.permissions.set(requestId, {
        resolve: (result) => resolve(result),
        timer,
        originalInput: input,
      })

      options.signal.addEventListener(
        'abort',
        () => {
          const pending = this.permissions.get(requestId)
          if (pending) {
            if (pending.timer) clearTimeout(pending.timer)
            this.permissions.delete(requestId)
            resolve({ behavior: 'deny', message: 'Request aborted' })
          }
        },
        { once: true },
      )

      emit({
        type: 'permission_request',
        requestId,
        toolName,
        toolInput: input,
        toolUseID: options.toolUseID,
        suggestions: options.suggestions,
        decisionReason: options.decisionReason,
        blockedPath: options.blockedPath,
        agentID: options.agentID,
        timeoutMs,
      } satisfies PermissionRequest)
    })
  }

  // ─── Resolve methods (called from WS handler) ──────────────────

  resolvePermission(
    requestId: string,
    allowed: boolean,
    updatedPermissions?: PermissionUpdate[],
  ): boolean {
    const pending = this.permissions.get(requestId)
    if (!pending) return false
    if (pending.timer) clearTimeout(pending.timer)
    this.permissions.delete(requestId)
    pending.resolve(
      allowed
        ? { behavior: 'allow', updatedInput: pending.originalInput, updatedPermissions }
        : { behavior: 'deny', message: 'User denied' },
    )
    return true
  }

  resolveQuestion(requestId: string, answers: Record<string, string>): boolean {
    const pending = this.questions.get(requestId)
    if (!pending) return false
    this.questions.delete(requestId)
    pending.resolve({ behavior: 'allow', updatedInput: { answers } })
    return true
  }

  resolvePlan(requestId: string, approved: boolean, feedback?: string): boolean {
    const pending = this.plans.get(requestId)
    if (!pending) return false
    this.plans.delete(requestId)
    pending.resolve(
      approved
        ? { behavior: 'allow', updatedInput: pending.originalInput }
        : { behavior: 'deny', message: feedback ?? 'Plan rejected' },
    )
    return true
  }

  resolveElicitation(requestId: string, response: string): boolean {
    const pending = this.elicitations.get(requestId)
    if (!pending) return false
    this.elicitations.delete(requestId)
    pending.resolve({ behavior: 'allow', updatedInput: { response } })
    return true
  }

  /** Drain all pending — deny permissions, reject plans, empty answers/responses */
  drainAll(): void {
    for (const [, p] of this.permissions) {
      if (p.timer) clearTimeout(p.timer)
      p.resolve({ behavior: 'deny', message: 'Session closing' })
    }
    this.permissions.clear()
    for (const [, p] of this.questions)
      p.resolve({ behavior: 'allow', updatedInput: { answers: {} } })
    this.questions.clear()
    for (const [, p] of this.plans) p.resolve({ behavior: 'deny', message: 'Session closing' })
    this.plans.clear()
    for (const [, p] of this.elicitations)
      p.resolve({ behavior: 'allow', updatedInput: { response: '' } })
    this.elicitations.clear()
  }

  /** Drain only interactive maps (questions/plans/elicitations) — for WS disconnect */
  drainInteractive(): void {
    for (const [, p] of this.questions)
      p.resolve({ behavior: 'allow', updatedInput: { answers: {} } })
    this.questions.clear()
    for (const [, p] of this.plans)
      p.resolve({ behavior: 'deny', message: 'Frontend disconnected' })
    this.plans.clear()
    for (const [, p] of this.elicitations)
      p.resolve({ behavior: 'allow', updatedInput: { response: '' } })
    this.elicitations.clear()
  }

  get pendingCount(): number {
    return this.permissions.size + this.questions.size + this.plans.size + this.elicitations.size
  }
}
