import { createContext, useContext } from 'react'

/**
 * Context to pass the in-session find query down to message components.
 * When a non-empty query is set, message text should be highlighted.
 */
const FindContext = createContext<string>('')

export const FindProvider = FindContext.Provider

export function useFindQuery(): string {
  return useContext(FindContext)
}
