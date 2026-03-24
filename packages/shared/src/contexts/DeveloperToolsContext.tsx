import { createContext, useContext, type ComponentType, type ReactNode } from 'react'

/**
 * Injectable developer-mode components.
 *
 * apps/web provides rich implementations (Shiki-highlighted code, interactive JSON tree).
 * When not provided (e.g. share viewer), conversation blocks fall back to plain <pre> rendering.
 */

export interface JsonTreeProps {
  data: unknown
  defaultExpandDepth?: number
  verboseMode?: boolean
}

export interface ToolRendererProps {
  inputData: Record<string, unknown>
  name: string
  blockIdPrefix?: string
}

interface DeveloperTools {
  JsonTree?: ComponentType<JsonTreeProps>
  getToolRenderer?: (name: string) => ComponentType<ToolRendererProps> | null
}

const DeveloperToolsContext = createContext<DeveloperTools>({})

export function DeveloperToolsProvider({
  children,
  value,
}: {
  children: ReactNode
  value: DeveloperTools
}) {
  return <DeveloperToolsContext.Provider value={value}>{children}</DeveloperToolsContext.Provider>
}

export function useDeveloperTools(): DeveloperTools {
  return useContext(DeveloperToolsContext)
}
