export interface InteractionErrorProps {
  variant: string
}

export function InteractionError({ variant }: InteractionErrorProps) {
  return (
    <div className="rounded-lg border border-red-200 dark:border-red-800/40 bg-red-50 dark:bg-red-950/30 p-3 text-sm text-red-700 dark:text-red-400">
      Could not display {variant} interaction
    </div>
  )
}
