// sidecar/src/workflow-runner.ts
//
// Workflow runner: executes multi-stage workflows defined in YAML.
// Each stage spawns a Claude Code session via the Agent SDK.
// Supports gate polling, retry, and crash recovery via on-disk pipeline state.

import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { parse as parseYaml } from 'yaml'

// ── Types ──

export interface WorkflowEvent {
  type:
    | 'stage_start'
    | 'attempt_start'
    | 'attempt_passed'
    | 'attempt_failed'
    | 'stage_passed'
    | 'workflow_failed'
    | 'workflow_complete'
  stage?: string
  attempt?: number
  summary?: string
}

interface StageDefinition {
  name: string
  prompt: string
  model?: string
  gate?: {
    retry?: boolean
    maxAttempts?: number
  }
}

interface WorkflowDefinition {
  name: string
  stages: StageDefinition[]
}

interface PipelineState {
  workflowId: string
  planFile: string
  currentStage: string
  attempt: number
  stageHistory: { stage: string; status: string; attempts: number }[]
}

// ── Constants ──

const RUNS_DIR = join(homedir(), '.claude-view', 'workflows', 'runs')

const VALID_WORKFLOW_ID = /^[a-zA-Z][a-zA-Z0-9_-]{0,63}$/

// ── Public API ──

export async function runWorkflow(
  workflowId: string,
  inputs: Record<string, string>,
  onEvent: (event: WorkflowEvent) => void,
): Promise<void> {
  if (!VALID_WORKFLOW_ID.test(workflowId)) {
    onEvent({
      type: 'workflow_failed',
      summary: `Invalid workflow ID: ${workflowId}`,
    })
    return
  }

  const yamlPath = resolveWorkflowPath(workflowId)
  if (!existsSync(yamlPath)) {
    onEvent({
      type: 'workflow_failed',
      summary: `Workflow definition not found: ${workflowId}`,
    })
    return
  }

  const definition = parseYaml(readFileSync(yamlPath, 'utf-8')) as WorkflowDefinition

  if (!definition.stages || definition.stages.length === 0) {
    onEvent({
      type: 'workflow_failed',
      summary: `Workflow "${workflowId}" has no stages defined.`,
    })
    return
  }

  const state = loadOrCreatePipeline(workflowId, inputs)

  // Resume from where we left off (crash recovery)
  for (let i = state.stageHistory.length; i < definition.stages.length; i++) {
    const stage = definition.stages[i]
    state.currentStage = stage.name
    onEvent({ type: 'stage_start', stage: stage.name })

    const maxAttempts = stage.gate?.maxAttempts ?? 1
    let passed = false

    while (!passed) {
      state.attempt++
      savePipeline(state)
      onEvent({
        type: 'attempt_start',
        stage: stage.name,
        attempt: state.attempt,
      })

      // TODO: spawn Claude Code session via Agent SDK (Phase F's spawnSession):
      // const session = await spawnSession({
      //   prompt: buildStagePrompt(stage, inputs),
      //   model: stage.model ?? 'claude-opus-4-6',
      // })
      // const result = await pollSessionUntilDone(session.id)

      // Stub for MVP — will be replaced with real Agent SDK calls:
      passed = true

      onEvent({
        type: passed ? 'attempt_passed' : 'attempt_failed',
        stage: stage.name,
        attempt: state.attempt,
      })

      if (!passed) {
        // Break if retries are disabled or max attempts reached
        if (!stage.gate?.retry || state.attempt >= maxAttempts) break
      }
    }

    state.stageHistory.push({
      stage: stage.name,
      status: passed ? 'passed' : 'failed',
      attempts: state.attempt,
    })
    state.attempt = 0

    if (!passed) {
      const failedAttempts = state.stageHistory[state.stageHistory.length - 1].attempts
      onEvent({
        type: 'workflow_failed',
        stage: stage.name,
        summary: `Stage "${stage.name}" failed after ${failedAttempts} attempt(s).`,
      })
      savePipeline(state)
      return
    }

    savePipeline(state)
    onEvent({ type: 'stage_passed', stage: stage.name })
  }

  onEvent({
    type: 'workflow_complete',
    summary: `All ${definition.stages.length} stage(s) passed.`,
  })
  cleanupPipeline(workflowId)
}

// ── Internal helpers ──

function resolveWorkflowPath(id: string): string {
  const userPath = join(homedir(), '.claude-view', 'workflows', 'user', `${id}.yaml`)
  if (existsSync(userPath)) return userPath
  return join(homedir(), '.claude-view', 'workflows', 'official', `${id}.yaml`)
}

function loadOrCreatePipeline(workflowId: string, inputs: Record<string, string>): PipelineState {
  mkdirSync(RUNS_DIR, { recursive: true })
  const path = join(RUNS_DIR, `${workflowId}.json`)
  if (existsSync(path)) {
    try {
      return JSON.parse(readFileSync(path, 'utf-8')) as PipelineState
    } catch (err) {
      console.warn(
        `[workflow-runner] Corrupt pipeline state for "${workflowId}", starting fresh:`,
        err,
      )
    }
  }
  return {
    workflowId,
    planFile: inputs.plan_file ?? '',
    currentStage: '',
    attempt: 0,
    stageHistory: [],
  }
}

function savePipeline(state: PipelineState): void {
  mkdirSync(RUNS_DIR, { recursive: true })
  writeFileSync(join(RUNS_DIR, `${state.workflowId}.json`), JSON.stringify(state, null, 2))
}

function cleanupPipeline(workflowId: string): void {
  const path = join(RUNS_DIR, `${workflowId}.json`)
  if (existsSync(path)) {
    unlinkSync(path)
  }
}
