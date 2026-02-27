import { useParams, useSearchParams } from 'react-router-dom'
import { DashboardChat } from '../components/live/DashboardChat'

export function ControlPage() {
  const { controlId } = useParams<{ controlId: string }>()
  const [searchParams] = useSearchParams()
  const sessionId = searchParams.get('sessionId')

  if (!controlId || !sessionId) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-gray-500 dark:text-gray-400">Missing control or session ID.</p>
      </div>
    )
  }

  return <DashboardChat controlId={controlId} sessionId={sessionId} />
}
