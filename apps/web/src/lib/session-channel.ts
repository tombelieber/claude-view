type SendFn = (msg: Record<string, unknown>) => void

interface PendingRequest {
  resolve: (data: unknown) => void
  reject: (err: Error) => void
  timer: ReturnType<typeof setTimeout>
}

export class SessionChannel {
  private sendFn: SendFn | null
  private pending = new Map<string, PendingRequest>()

  constructor(sendFn: SendFn | null) {
    this.sendFn = sendFn
  }

  /** Update the send function (e.g., on WS reconnect). */
  updateSend(sendFn: SendFn | null): void {
    this.sendFn = sendFn
  }

  /** Fire-and-forget. No-op if not connected. */
  send(msg: Record<string, unknown>): void {
    this.sendFn?.(msg)
  }

  /**
   * Request/response with per-request requestId correlation.
   * Rejects on timeout or disconnect. No deduplication.
   */
  request<T>(msg: Record<string, unknown>, timeoutMs = 10_000): Promise<T> {
    if (!this.sendFn) {
      return Promise.reject(new Error('SessionChannel not connected'))
    }

    const requestId = crypto.randomUUID()
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(requestId)
        reject(new Error(`Request timeout after ${timeoutMs}ms`))
      }, timeoutMs)

      this.pending.set(requestId, {
        resolve: resolve as (data: unknown) => void,
        reject,
        timer,
      })

      this.sendFn?.({ ...msg, requestId })
    })
  }

  /** Resolve a pending request by requestId. Called by the WS event router. */
  handleResponse(requestId: string, data: unknown): void {
    const entry = this.pending.get(requestId)
    if (!entry) return
    clearTimeout(entry.timer)
    this.pending.delete(requestId)
    entry.resolve(data)
  }

  /** Reject all pending requests. Called on WS disconnect. */
  handleDisconnect(): void {
    for (const [, entry] of this.pending) {
      clearTimeout(entry.timer)
      entry.reject(new Error('SessionChannel disconnect'))
    }
    this.pending.clear()
  }
}
