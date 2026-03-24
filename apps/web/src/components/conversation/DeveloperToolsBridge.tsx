import { DeveloperToolsProvider } from '@claude-view/shared/contexts/DeveloperToolsContext'
import type { ReactNode } from 'react'
import { JsonTree } from '../live/JsonTree'
import { getToolRenderer } from '../live/ToolRenderers'

/**
 * Bridges apps/web's rich developer tools (JsonTree with Shiki, ToolRenderers)
 * into the shared conversation components via DeveloperToolsContext.
 *
 * Wrap any ConversationThread rendering with this provider to get
 * rich JSON trees and tool-specific renderers in developer mode.
 */
export function DeveloperToolsBridge({ children }: { children: ReactNode }) {
  return (
    <DeveloperToolsProvider value={{ JsonTree, getToolRenderer }}>
      {children}
    </DeveloperToolsProvider>
  )
}
