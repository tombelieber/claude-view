import type { z } from 'zod'
import type { ClaudeViewClient } from '../client.js'

export interface ToolDef<TSchema extends z.ZodObject<z.ZodRawShape> = z.ZodObject<z.ZodRawShape>> {
  name: string
  description: string
  inputSchema: TSchema
  annotations: Record<string, boolean>
  handler: (client: ClaudeViewClient, args: z.output<TSchema>) => Promise<string>
}
