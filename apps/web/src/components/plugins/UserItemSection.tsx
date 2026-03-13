import type { UserItemInfo } from '../../types/generated'
import { SectionHeader } from './SectionHeader'
import { UserItemCard } from './UserItemCard'

interface UserItemSectionProps {
  title: string
  items: UserItemInfo[]
  pathPrefix: string
}

export function UserItemSection({ title, items, pathPrefix }: UserItemSectionProps) {
  return (
    <div>
      <SectionHeader title={title} count={items.length} pathHint={pathPrefix} />
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {items.map((item) => (
          <UserItemCard key={item.path} item={item} />
        ))}
      </div>
    </div>
  )
}
