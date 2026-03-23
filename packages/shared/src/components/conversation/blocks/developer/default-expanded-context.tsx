import { createContext, useContext } from 'react'

/**
 * When true, all collapsible cards (ToolCard, etc.) start expanded.
 * Used by gallery stories to show full content without manual clicks.
 */
const DefaultExpandedContext = createContext(false)

export const DefaultExpandedProvider = DefaultExpandedContext.Provider
export const useDefaultExpanded = () => useContext(DefaultExpandedContext)
