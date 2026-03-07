/**
 * CodeRenderContext -- lets apps inject their own code block renderers.
 *
 * Shared components (MessageTyped, BashProgressCard, etc.) call useCodeBlock()
 * and useCompactCodeBlock() to get the active renderer. This allows apps/web to
 * provide shiki-highlighted versions while apps/share uses the plain fallback.
 *
 * If no provider is mounted, the default plain-text components from
 * packages/shared are used automatically.
 */

import { type ComponentType, type ReactNode, createContext, useContext } from 'react'
import { CodeBlock as DefaultCodeBlock } from '../components/CodeBlock'
import { CompactCodeBlock as DefaultCompactCodeBlock } from '../components/CompactCodeBlock'

export interface CodeBlockProps {
  code: string | null | undefined
  language?: string | null | undefined
  blockId?: string
}

export interface CompactCodeBlockProps {
  code: string | null | undefined
  language?: string | null | undefined
  blockId?: string
}

export interface CodeRenderContextValue {
  CodeBlock: ComponentType<CodeBlockProps>
  CompactCodeBlock: ComponentType<CompactCodeBlockProps>
}

const defaultValue: CodeRenderContextValue = {
  CodeBlock: DefaultCodeBlock,
  CompactCodeBlock: DefaultCompactCodeBlock,
}

const CodeRenderContext = createContext<CodeRenderContextValue>(defaultValue)

export function CodeRenderProvider({
  value,
  children,
}: {
  value: CodeRenderContextValue
  children: ReactNode
}) {
  return <CodeRenderContext.Provider value={value}>{children}</CodeRenderContext.Provider>
}

/**
 * Returns the app-provided CodeBlock component, or the shared plain fallback.
 */
export function useCodeBlock(): ComponentType<CodeBlockProps> {
  return useContext(CodeRenderContext).CodeBlock
}

/**
 * Returns the app-provided CompactCodeBlock component, or the shared plain fallback.
 */
export function useCompactCodeBlock(): ComponentType<CompactCodeBlockProps> {
  return useContext(CodeRenderContext).CompactCodeBlock
}
