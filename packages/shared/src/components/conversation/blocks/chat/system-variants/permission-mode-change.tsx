import { Shield } from 'lucide-react'
import { StatusBadge } from '../../shared/StatusBadge'

interface Props {
  data: Record<string, unknown>
}

export function PermissionModeChangePill({ data }: Props) {
  const mode = (data?.permissionMode as string) ?? ''
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Shield className="w-3 h-3 flex-shrink-0" />
      <span>Permission mode:</span>
      {mode && <StatusBadge label={mode} color="amber" />}
    </div>
  )
}
