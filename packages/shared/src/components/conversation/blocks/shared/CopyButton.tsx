import { Check, Copy } from 'lucide-react'
import { useCallback, useState } from 'react'

interface CopyButtonProps {
  text: string
  className?: string
}

/** Compact copy-to-clipboard button with check feedback. Min 44px tap target. */
export function CopyButton({ text, className }: CopyButtonProps) {
  const [copied, setCopied] = useState(false)
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [text])
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation()
        handleCopy()
      }}
      className={`min-w-[28px] min-h-[28px] inline-flex items-center justify-center rounded transition-colors duration-200 cursor-pointer text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50 ${className ?? ''}`}
      title="Copy to clipboard"
      aria-label={copied ? 'Copied' : 'Copy to clipboard'}
    >
      {copied ? (
        <Check className="w-3 h-3 text-green-500 dark:text-green-400" />
      ) : (
        <Copy className="w-3 h-3" />
      )}
    </button>
  )
}
