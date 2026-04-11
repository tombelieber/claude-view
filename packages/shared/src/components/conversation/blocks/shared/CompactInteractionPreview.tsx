export interface PendingInteractionMeta {
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId: string
  preview: string
}

const VARIANT_LABELS: Record<string, { label: string; color: string }> = {
  permission: { label: 'Permission', color: 'text-amber-600 dark:text-amber-400' },
  question: { label: 'Question', color: 'text-blue-600 dark:text-blue-400' },
  plan: { label: 'Plan', color: 'text-purple-600 dark:text-purple-400' },
  elicitation: { label: 'Input', color: 'text-green-600 dark:text-green-400' },
}

export function CompactInteractionPreview({ meta }: { meta: PendingInteractionMeta }) {
  const config = VARIANT_LABELS[meta.variant] ?? {
    label: meta.variant,
    color: 'text-gray-600 dark:text-gray-400',
  }

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-3 animate-pulse">
      <span className={`text-xs font-medium ${config.color}`}>{config.label}</span>
      <p className="mt-1 text-sm text-gray-600 dark:text-gray-400 truncate">{meta.preview}</p>
    </div>
  )
}
