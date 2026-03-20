import { createContext, useContext } from 'react'

type SidecarIdsContextValue = {
  addLocalSidecarId: (sessionId: string) => void
}

const SidecarIdsContext = createContext<SidecarIdsContextValue>({
  addLocalSidecarId: () => {},
})

export const SidecarIdsProvider = SidecarIdsContext.Provider
export const useSidecarIds = () => useContext(SidecarIdsContext)
