function formatTokens(n: number): string {
  return n.toLocaleString('en-US')
}

interface ChatStatusBarProps {
  model: string
  contextTokens: number
  contextLimit: number
  contextPercent: number
  totalCost: number | null
}

export function ChatStatusBar({
  model,
  contextTokens,
  contextLimit,
  contextPercent,
  totalCost,
}: ChatStatusBarProps) {
  return (
    <div
      className="flex items-center justify-between px-3 py-1 text-xs
        bg-[#f6f8fa] dark:bg-[#161B22]
        border-t border-gray-200 dark:border-[#30363D]
        text-gray-500 dark:text-[#8B949E]"
    >
      <span>{model}</span>
      <span>
        {formatTokens(contextTokens)} / {formatTokens(contextLimit)} ({contextPercent}%)
      </span>
      {totalCost != null && <span>${totalCost.toFixed(2)} USD</span>}
    </div>
  )
}
