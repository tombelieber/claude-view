import { AlertCircle } from 'lucide-react'

interface ErrorBlockProps {
  message: string
}

export function ErrorBlock({ message }: ErrorBlockProps) {
  return (
    <div className="flex items-start gap-2 px-3 py-2 bg-red-50 dark:bg-red-950/20 border border-red-200 dark:border-red-800/40 rounded-md">
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 shrink-0 mt-0.5" />
      <p className="text-xs text-red-700 dark:text-red-300">{message}</p>
    </div>
  )
}
