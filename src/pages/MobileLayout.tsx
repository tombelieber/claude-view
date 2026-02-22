import { Outlet } from 'react-router-dom'

/** Minimal layout for /mobile — no desktop header, sidebar, or status bar. */
export function MobileLayout() {
  return (
    <div className="h-screen flex flex-col bg-gray-950 text-gray-100">
      <Outlet />
    </div>
  )
}
