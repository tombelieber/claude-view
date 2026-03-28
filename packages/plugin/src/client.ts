export class ClaudeViewClient {
  readonly baseUrl: string

  constructor(port?: number) {
    const resolvedPort = port ?? (Number(process.env.CLAUDE_VIEW_PORT) || 47892)
    this.baseUrl = `http://localhost:${resolvedPort}`
  }

  async get<T = unknown>(
    path: string,
    params?: Record<string, string | number | undefined>,
  ): Promise<T> {
    const url = new URL(path, this.baseUrl)
    if (params) {
      for (const [key, value] of Object.entries(params)) {
        if (value !== undefined) {
          url.searchParams.set(key, String(value))
        }
      }
    }

    let response: Response
    try {
      response = await fetch(url.toString(), {
        headers: { Accept: 'application/json' },
        signal: AbortSignal.timeout(10_000),
      })
    } catch {
      throw new Error(
        `claude-view server not detected at ${this.baseUrl}. Start it with: npx claude-view`,
      )
    }

    if (!response.ok) {
      const body = await response.text().catch(() => '')
      throw new Error(`claude-view API error ${response.status}: ${body}`)
    }

    return response.json() as Promise<T>
  }

  async request<T = unknown>(
    method: string,
    path: string,
    options?: {
      params?: Record<string, string | number | undefined>
      body?: unknown
      timeout?: number
    },
  ): Promise<T> {
    const url = new URL(path, this.baseUrl)
    if (options?.params) {
      for (const [key, value] of Object.entries(options.params)) {
        if (value !== undefined) {
          url.searchParams.set(key, String(value))
        }
      }
    }

    let response: Response
    try {
      const fetchOpts: RequestInit = {
        method,
        headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
        signal: AbortSignal.timeout(options?.timeout ?? 10_000),
      }
      if (options?.body !== undefined) {
        fetchOpts.body = JSON.stringify(options.body)
      }
      response = await fetch(url.toString(), fetchOpts)
    } catch {
      throw new Error(
        `claude-view server not detected at ${this.baseUrl}. Start it with: npx claude-view`,
      )
    }

    if (!response.ok) {
      const body = await response.text().catch(() => '')
      throw new Error(`claude-view API error ${response.status}: ${body}`)
    }

    return response.json() as Promise<T>
  }
}
