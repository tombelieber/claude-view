import { createContext, useContext } from 'react'

/**
 * Global JSON mode for Developer view.
 * When true, all EventCards and ToolCards show raw JSON instead of tailored UI.
 */
const JsonModeContext = createContext(false)

export const JsonModeProvider = JsonModeContext.Provider
export const useJsonMode = () => useContext(JsonModeContext)
