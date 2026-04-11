import { HelpCircle } from 'lucide-react'
import { CollapsibleJson } from '../../shared/CollapsibleJson'

interface Props {
  data: Record<string, unknown>
  sdkType?: string
}

export function UnknownSystemPill({ data, sdkType }: Props) {
  const label = sdkType ?? (data?.type as string) ?? 'unknown'
  return (
    <div className="px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <div className="flex items-center gap-2 mb-1">
        <HelpCircle className="w-3 h-3 flex-shrink-0" />
        <span className="font-mono">{label}</span>
      </div>
      <CollapsibleJson data={data} label="data" />
    </div>
  )
}
