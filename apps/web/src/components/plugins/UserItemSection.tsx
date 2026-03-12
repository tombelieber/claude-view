import { useState } from 'react'
import type { UserItemInfo } from '../../types/generated'
import { SectionHeader } from './SectionHeader'
import { UserItemCard } from './UserItemCard'

interface UserItemSectionProps {
  title: string
  items: UserItemInfo[]
  pathPrefix: string
}

const MAX_VISIBLE = 3

export function UserItemSection({ title, items, pathPrefix }: UserItemSectionProps) {
  const [expanded, setExpanded] = useState(false)
  const visible = expanded ? items : items.slice(0, MAX_VISIBLE)
  const remaining = items.length - MAX_VISIBLE

  return (
    <div>
      <SectionHeader title={title} count={items.length} pathHint={pathPrefix} />
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {visible.map((item) => (
          <UserItemCard key={item.path} item={item} />
        ))}
        {remaining > 0 && !expanded && (
          <button
            type="button"
            onClick={() => setExpanded(true)}
            className="flex items-center justify-center min-h-[66px] rounded-xl border border-dashed border-apple-sep2 bg-apple-bg hover:border-apple-blue transition-colors cursor-pointer"
          >
            <span className="text-[13px] text-apple-text3">
              + {remaining} more {title.toLowerCase()}
            </span>
          </button>
        )}
      </div>
    </div>
  )
}
