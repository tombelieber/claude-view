import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'

interface ExpandContextType {
  expandedBlocks: Set<string>
  toggleBlock: (id: string) => void
}

const ExpandContext = createContext<ExpandContextType>({
  expandedBlocks: new Set(),
  toggleBlock: () => {},
})

export function useExpandContext() {
  return useContext(ExpandContext)
}

export function ExpandProvider({ children }: { children: ReactNode }) {
  const [expandedBlocks, setExpandedBlocks] = useState<Set<string>>(new Set())

  const toggleBlock = useCallback((id: string) => {
    setExpandedBlocks(prev => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }, [])

  return (
    <ExpandContext.Provider value={{ expandedBlocks, toggleBlock }}>
      {children}
    </ExpandContext.Provider>
  )
}
