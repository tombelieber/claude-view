#!/usr/bin/env bun
/**
 * Reads openapi.json and generates MCP tool files per OpenAPI tag.
 *
 * Usage:  bun run scripts/codegen-from-openapi.ts
 *
 * Output: src/tools/generated/<tag>.ts  (one file per tag)
 *         src/tools/generated/index.ts  (barrel re-export)
 */

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { HAND_WRITTEN_TAGS, SSE_OPERATION_IDS, makeToolName } from './shared.js'

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const ROOT = dirname(dirname(new URL(import.meta.url).pathname))
const SPEC_PATH = join(ROOT, 'scripts', 'openapi.json')
const GEN_DIR = join(ROOT, 'src', 'tools', 'generated')

// operationIds that duplicate hand-written tools — skip generation
const SKIP_OPERATION_IDS = new Set([
  'get_fluency_score', // duplicates hand-written get_fluency_score in stats.ts
  'search_handler', // duplicates hand-written search_sessions in sessions.ts
])

// POST/PUT operations that are destructive despite not being DELETE
const FORCE_DESTRUCTIVE_OPS = new Set([
  'reset_all',
  'clear_cache',
  'git_resync',
  'trigger_reindex',
  'kill_process',
  'cleanup_processes',
])

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface OpenAPISpec {
  paths: Record<string, Record<string, OpenAPIOperation>>
}

interface OpenAPIOperation {
  tags?: string[]
  summary?: string
  description?: string
  operationId?: string
  parameters?: OpenAPIParameter[]
  requestBody?: {
    content?: Record<string, { schema?: SchemaObject }>
    required?: boolean
  }
  responses?: Record<string, { content?: Record<string, unknown> }>
}

interface OpenAPIParameter {
  name: string
  in: 'path' | 'query' | 'header' | 'cookie'
  required?: boolean
  schema?: SchemaObject
  description?: string
}

interface SchemaObject {
  type?: string | string[]
  properties?: Record<string, SchemaObject>
  required?: string[]
  items?: SchemaObject
  description?: string
  enum?: string[]
  format?: string
  $ref?: string
}

interface EndpointInfo {
  method: string
  path: string
  operationId: string
  summary: string
  description: string
  pathParams: ParamInfo[]
  queryParams: ParamInfo[]
  bodyParams: ParamInfo[]
  hasBody: boolean
}

interface ParamInfo {
  name: string
  zodType: string
  required: boolean
  description: string
}

// ---------------------------------------------------------------------------
// OpenAPI -> Zod helpers
// ---------------------------------------------------------------------------

function resolveRef(spec: OpenAPISpec, ref: string): SchemaObject {
  const parts = ref.replace('#/', '').split('/')
  let current: any = spec
  for (const part of parts) {
    current = current?.[part]
  }
  return (current as SchemaObject) ?? {}
}

function normalizeType(typeField: string | string[] | undefined): string {
  if (!typeField) return 'string'
  if (Array.isArray(typeField)) {
    // e.g. ['string', 'null'] -> 'string'
    const nonNull = typeField.filter((t) => t !== 'null')
    return nonNull[0] ?? 'string'
  }
  return typeField
}

function schemaToZod(schema: SchemaObject | undefined, required: boolean): string {
  if (!schema) return required ? 'z.unknown()' : 'z.unknown().optional()'
  const baseType = normalizeType(schema.type)
  let zod: string
  switch (baseType) {
    case 'integer':
    case 'number':
      zod = 'z.number()'
      break
    case 'boolean':
      zod = 'z.boolean()'
      break
    case 'array':
      zod = 'z.array(z.unknown())'
      break
    case 'object':
      zod = 'z.record(z.unknown())'
      break
    default:
      zod = 'z.string()'
  }
  if (schema.description) {
    const escaped = schema.description
      .replace(/\n/g, ' ')
      .replace(/\s+/g, ' ')
      .replace(/\\/g, '\\\\')
      .replace(/'/g, "\\'")
      .trim()
    zod += `.describe('${escaped}')`
  }
  if (!required) {
    zod += '.optional()'
  }
  return zod
}

// ---------------------------------------------------------------------------
// Parse one operation into EndpointInfo
// ---------------------------------------------------------------------------

function parseOperation(
  spec: OpenAPISpec,
  method: string,
  path: string,
  op: OpenAPIOperation,
): EndpointInfo {
  const operationId = op.operationId ?? `${method}_${path.replace(/[^a-zA-Z0-9]/g, '_')}`
  const summary = op.summary ?? ''
  const description = op.description ?? summary

  const pathParams: ParamInfo[] = []
  const queryParams: ParamInfo[] = []

  for (const param of op.parameters ?? []) {
    if (param.in !== 'path' && param.in !== 'query') continue
    const info: ParamInfo = {
      name: param.name,
      zodType: schemaToZod(param.schema, param.required ?? false),
      required: param.required ?? false,
      description: param.description ?? '',
    }
    if (param.in === 'path') {
      // Path params are always required
      info.required = true
      info.zodType = schemaToZod(param.schema, true)
      pathParams.push(info)
    } else {
      // Query params are always optional at the MCP level — backends must
      // handle their absence even if OpenAPI marks them required
      info.required = false
      info.zodType = schemaToZod(param.schema, false)
      queryParams.push(info)
    }
  }

  const bodyParams: ParamInfo[] = []
  let hasBody = false

  if (op.requestBody?.content) {
    const jsonContent = op.requestBody.content['application/json']
    if (jsonContent?.schema) {
      let bodySchema = jsonContent.schema
      if (bodySchema.$ref) {
        bodySchema = resolveRef(spec, bodySchema.$ref)
      }
      if (bodySchema.properties) {
        hasBody = true
        const requiredFields = new Set(bodySchema.required ?? [])
        for (const [propName, propSchema] of Object.entries(bodySchema.properties)) {
          let resolved = propSchema
          if (resolved.$ref) {
            resolved = resolveRef(spec, resolved.$ref)
          }
          bodyParams.push({
            name: propName,
            zodType: schemaToZod(resolved, requiredFields.has(propName)),
            required: requiredFields.has(propName),
            description: resolved.description ?? '',
          })
        }
      }
    }
  }

  return {
    method,
    path,
    operationId,
    summary,
    description,
    pathParams,
    queryParams,
    bodyParams,
    hasBody,
  }
}

// ---------------------------------------------------------------------------
// Codegen per file
// ---------------------------------------------------------------------------

// toSnakeCase and makeToolName imported from ./shared.js

function safePropertyName(name: string): string {
  // If name contains characters invalid for JS identifiers, quote it
  if (/^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(name)) return name
  return `'${name}'`
}

function generateToolCode(tag: string, endpoint: EndpointInfo): string {
  const toolName = makeToolName(tag, endpoint.operationId)
  const allParams = [...endpoint.pathParams, ...endpoint.queryParams, ...endpoint.bodyParams]

  // Build description: prefer the short summary, fall back to description
  let desc =
    endpoint.summary || endpoint.description || `${endpoint.method.toUpperCase()} ${endpoint.path}`
  // Strip the method prefix pattern like "GET /api/foo — " that the spec often includes
  desc = desc.replace(/^(GET|POST|PUT|DELETE|PATCH)\s+\/api\/\S+\s*[-—]\s*/i, '').trim()
  // If description is still just a raw HTTP method+path, synthesize from operationId
  if (!desc || /^(GET|POST|PUT|DELETE|PATCH)\s+\/api\//i.test(desc)) {
    desc = endpoint.operationId
      .replace(/_/g, ' ')
      .replace(/\b\w/g, (c) => c.toUpperCase())
      .replace(/^(.)/, (c) => c.toUpperCase())
  }
  // If description is very short (≤2 words), synthesize a richer one from operationId + path
  if (desc.split(/\s+/).length <= 2) {
    // Use operationId as the base — it's always meaningful (e.g. "list_workflows", "create_workflow")
    const fromOpId = endpoint.operationId
      .replace(/_/g, ' ')
      .replace(/\b\w/g, (c) => c.toUpperCase())
    // Append the resource path for context (e.g. "via /api/workflows")
    const resource = endpoint.path
      .replace('/api/', '')
      .split('/')
      .filter((p) => !p.startsWith('{'))
      .join('/')
    desc = `${fromOpId} (${endpoint.method.toUpperCase()} /api/${resource})`
  }
  desc = desc
    .replace(/\n/g, ' ')
    .replace(/\s+/g, ' ')
    .replace(/\\/g, '\\\\')
    .replace(/'/g, "\\'")
    .trim()

  // Schema properties
  const schemaLines: string[] = []
  for (const p of allParams) {
    schemaLines.push(`    ${safePropertyName(p.name)}: ${p.zodType},`)
  }

  const schemaStr =
    schemaLines.length > 0 ? `z.object({\n${schemaLines.join('\n')}\n  })` : 'z.object({})'

  // Annotations
  const isReadOnly = endpoint.method === 'get'
  const isDestructive =
    endpoint.method === 'delete' || FORCE_DESTRUCTIVE_OPS.has(endpoint.operationId)

  // Handler body — build the path with interpolation for path params
  // Wrap each path param with encodeURIComponent to prevent path traversal
  let pathExpr: string
  if (endpoint.pathParams.length > 0) {
    let interpolated = endpoint.path
    for (const p of endpoint.pathParams) {
      interpolated = interpolated.replace(
        `{${p.name}}`,
        `\${encodeURIComponent(String(args.${p.name}))}`,
      )
    }
    pathExpr = `\`${interpolated}\``
  } else {
    pathExpr = `'${endpoint.path}'`
  }

  // Query params object
  const queryParamNames = endpoint.queryParams.map((p) => p.name)
  const queryObj =
    queryParamNames.length > 0
      ? `{ ${queryParamNames.map((n) => `${safePropertyName(n)}: args.${n}`).join(', ')} }`
      : undefined

  // Body object
  const bodyParamNames = endpoint.bodyParams.map((p) => p.name)
  const bodyObj =
    bodyParamNames.length > 0
      ? `{ ${bodyParamNames.map((n) => `${safePropertyName(n)}: args.${n}`).join(', ')} }`
      : undefined

  // Build options argument
  const optsParts: string[] = []
  if (queryObj) optsParts.push(`params: ${queryObj}`)
  if (bodyObj) optsParts.push(`body: ${bodyObj}`)
  const optsStr = optsParts.length > 0 ? `, { ${optsParts.join(', ')} }` : ''

  return `  {
    name: '${toolName}',
    description: '${desc}',
    inputSchema: ${schemaStr},
    annotations: { readOnlyHint: ${isReadOnly}, destructiveHint: ${isDestructive}, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('${endpoint.method.toUpperCase()}', ${pathExpr}${optsStr})
      return JSON.stringify(result, null, 2)
    },
  }`
}

function generateTagFile(tag: string, endpoints: EndpointInfo[]): string {
  const exportName = `${tag.replace(/[^a-zA-Z0-9]/g, '')}GeneratedTools`
  const toolBlocks = endpoints.map((ep) => generateToolCode(tag, ep))

  return `// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const ${exportName}: ToolDef[] = [
${toolBlocks.join(',\n')}
]
`
}

function generateIndex(tagExports: { tag: string; exportName: string }[]): string {
  const imports = tagExports
    .map((t) => `import { ${t.exportName} } from './${t.tag}.js'`)
    .join('\n')

  const spread = tagExports.map((t) => `  ...${t.exportName},`).join('\n')

  return `// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import type { ToolDef } from '../types.js'
${imports}

export const allGeneratedTools: ToolDef[] = [
${spread}
]
`
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const spec = JSON.parse(readFileSync(SPEC_PATH, 'utf-8')) as OpenAPISpec

  // Group endpoints by tag
  const byTag = new Map<string, EndpointInfo[]>()

  for (const [path, methods] of Object.entries(spec.paths)) {
    for (const [method, op] of Object.entries(methods)) {
      if (!['get', 'post', 'put', 'delete', 'patch'].includes(method)) continue

      const operation = op as OpenAPIOperation
      const tag = (operation.tags?.[0] ?? 'misc').toLowerCase()
      const operationId = operation.operationId ?? ''

      // Skip SSE endpoints
      if (SSE_OPERATION_IDS.has(operationId)) {
        continue
      }

      // Skip operationIds that duplicate hand-written tools
      if (SKIP_OPERATION_IDS.has(operationId)) {
        continue
      }

      // Skip hand-written tags
      if (HAND_WRITTEN_TAGS.has(tag)) {
        continue
      }

      const endpoint = parseOperation(spec, method, path, operation)

      if (!byTag.has(tag)) byTag.set(tag, [])
      byTag.get(tag)?.push(endpoint)
    }
  }

  // Clean and recreate generated dir
  if (existsSync(GEN_DIR)) {
    rmSync(GEN_DIR, { recursive: true })
  }
  mkdirSync(GEN_DIR, { recursive: true })

  // Generate per-tag files
  const tagExports: { tag: string; exportName: string }[] = []
  let _totalTools = 0

  for (const [tag, endpoints] of [...byTag.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    const exportName = `${tag.replace(/[^a-zA-Z0-9]/g, '')}GeneratedTools`
    const content = generateTagFile(tag, endpoints)
    const outPath = join(GEN_DIR, `${tag}.ts`)
    writeFileSync(outPath, content)
    tagExports.push({ tag, exportName })
    _totalTools += endpoints.length
  }

  // Generate index
  const indexContent = generateIndex(tagExports)
  writeFileSync(join(GEN_DIR, 'index.ts'), indexContent)
}

main()
