/**
 * Build a session detail URL preserving sidebar filter params (project, branch).
 */
export function buildSessionUrl(sessionId: string, searchParams?: URLSearchParams): string {
  const encodedId = encodeURIComponent(sessionId)
  const basePath = `/sessions/${encodedId}`

  const params = searchParams ?? new URLSearchParams(window.location.search)

  const preserved = new URLSearchParams()
  const project = params.get('project')
  const branch = params.get('branch')
  if (project) preserved.set('project', project)
  if (branch) preserved.set('branch', branch)

  const qs = preserved.toString()
  return qs ? `${basePath}?${qs}` : basePath
}
