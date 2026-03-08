import { Users } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { TeamCard } from '../components/teams/TeamCard'
import { useTeams } from '../hooks/use-teams'

export function TeamsPage() {
  const navigate = useNavigate()
  const { data: teams, isLoading, error } = useTeams()

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      <div className="px-6 pt-6 pb-4">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Teams</h1>
          {teams && (
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {teams.length} team runs
            </span>
          )}
        </div>
      </div>

      <div className="flex-1 px-6 pb-6">
        {isLoading && (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="h-36 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse" />
            ))}
          </div>
        )}

        {error && <div className="text-sm text-red-500">Failed to load teams: {error.message}</div>}

        {teams && teams.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-gray-400 dark:text-gray-500">
            <Users className="w-10 h-10 mb-3 opacity-40" />
            <p className="text-sm font-medium">No teams found</p>
            <p className="text-xs mt-1">
              Teams appear here when you use Claude Code's /team command.
            </p>
          </div>
        )}

        {teams && teams.length > 0 && (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {teams.map((team) => (
              <TeamCard
                key={team.name}
                team={team}
                onClick={() => navigate(`/sessions/${team.leadSessionId}?tab=teams`)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
