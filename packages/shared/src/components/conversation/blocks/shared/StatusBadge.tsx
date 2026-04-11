type BadgeColor =
  | 'gray'
  | 'green'
  | 'red'
  | 'amber'
  | 'blue'
  | 'cyan'
  | 'teal'
  | 'orange'
  | 'indigo'
  | 'violet'
  | 'purple'

interface StatusBadgeProps {
  label: string
  color?: BadgeColor
}

const COLOR_CLASSES: Record<BadgeColor, string> = {
  gray: 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-600 dark:text-gray-400',
  green: 'bg-green-500/10 dark:bg-green-500/20 text-green-600 dark:text-green-400',
  red: 'bg-red-500/10 dark:bg-red-500/20 text-red-600 dark:text-red-400',
  amber: 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-600 dark:text-amber-400',
  blue: 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400',
  cyan: 'bg-cyan-500/10 dark:bg-cyan-500/20 text-cyan-600 dark:text-cyan-400',
  teal: 'bg-teal-500/10 dark:bg-teal-500/20 text-teal-600 dark:text-teal-400',
  orange: 'bg-orange-500/10 dark:bg-orange-500/20 text-orange-600 dark:text-orange-400',
  indigo: 'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-600 dark:text-indigo-400',
  violet: 'bg-violet-500/10 dark:bg-violet-500/20 text-violet-600 dark:text-violet-400',
  purple: 'bg-purple-500/10 dark:bg-purple-500/20 text-purple-600 dark:text-purple-400',
}

export function StatusBadge({ label, color = 'gray' }: StatusBadgeProps) {
  return (
    <span className={`text-xs font-mono px-1.5 py-0.5 rounded ${COLOR_CLASSES[color]}`}>
      {label}
    </span>
  )
}
