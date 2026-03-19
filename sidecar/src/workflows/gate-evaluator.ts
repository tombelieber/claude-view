import { existsSync } from 'node:fs'
import { getNestedField } from './get-nested-field.js'
import type { GateCondition } from './workflow-schema.js'

/**
 * Evaluates a gate condition against the agent's text output.
 * Returns true if the gate passes, false otherwise.
 * Throws on malformed input (e.g. invalid JSON for json_field, invalid regex pattern).
 */
export function evaluateGate(gate: GateCondition, agentOutput: string): boolean {
  switch (gate.type) {
    case 'json_field': {
      const parsed = JSON.parse(agentOutput)
      const actual = getNestedField(parsed, gate.field)
      switch (gate.operator) {
        case 'equals':
          return actual === gate.value
        case 'contains':
          return String(actual).includes(String(gate.value))
        case 'gt':
          return Number(actual) > Number(gate.value)
        case 'lt':
          return Number(actual) < Number(gate.value)
        default:
          return false
      }
    }
    case 'regex':
      return new RegExp(gate.pattern).test(agentOutput)
    case 'exit_code':
      return agentOutput.includes(`exit code ${gate.value}`)
    case 'file_exists':
      return existsSync(gate.path)
    default:
      return true
  }
}
