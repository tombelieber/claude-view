interface SectionHeaderProps {
  title: string
  count: number
  pathHint?: string
}

export function SectionHeader({ title, count, pathHint }: SectionHeaderProps) {
  return (
    <div className="flex items-center gap-2.5 mb-3">
      <span className="text-xs font-bold uppercase tracking-[0.08em] text-apple-text2 whitespace-nowrap">
        {title}
      </span>
      <span className="text-xs text-apple-text3 bg-apple-bg border border-apple-sep2 px-1.5 py-px rounded-full font-medium">
        {count}
      </span>
      <div className="flex-1 h-px bg-apple-sep2" />
      {pathHint && (
        <span className="text-xs text-apple-text3 whitespace-nowrap font-mono">{pathHint}</span>
      )}
    </div>
  )
}
