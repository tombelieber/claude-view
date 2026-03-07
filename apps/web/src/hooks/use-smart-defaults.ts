import { useMemo } from 'react'

export interface CardConfig {
  label: string
  dateStart: string
  dateEnd: string
  type: 'daily' | 'weekly'
  startTs: number
  endTs: number
}

export interface SmartDefaults {
  cards: CardConfig[]
  suggestedIndex: number
}

function toLocalDate(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function startOfDay(d: Date): number {
  const copy = new Date(d)
  copy.setHours(0, 0, 0, 0)
  return Math.floor(copy.getTime() / 1000)
}

function endOfDay(d: Date): number {
  const copy = new Date(d)
  copy.setHours(23, 59, 59, 999)
  return Math.floor(copy.getTime() / 1000)
}

function getMonday(d: Date): Date {
  const copy = new Date(d)
  const day = copy.getDay()
  const diff = day === 0 ? -6 : 1 - day
  copy.setDate(copy.getDate() + diff)
  copy.setHours(0, 0, 0, 0)
  return copy
}

export function useSmartDefaults(): SmartDefaults {
  return useMemo(() => {
    const now = new Date()
    const hour = now.getHours()
    const dayOfWeek = now.getDay()

    const today = new Date(now)
    today.setHours(0, 0, 0, 0)

    const yesterday = new Date(today)
    yesterday.setDate(yesterday.getDate() - 1)

    const thisMonday = getMonday(today)
    const lastMonday = new Date(thisMonday)
    lastMonday.setDate(lastMonday.getDate() - 7)
    const lastSunday = new Date(thisMonday)
    lastSunday.setDate(lastSunday.getDate() - 1)

    // Fixed order: Today, Yesterday, This Week, Last Week
    const cards: CardConfig[] = [
      {
        label: 'Today',
        dateStart: toLocalDate(today),
        dateEnd: toLocalDate(today),
        type: 'daily',
        startTs: startOfDay(today),
        endTs: endOfDay(today),
      },
      {
        label: 'Yesterday',
        dateStart: toLocalDate(yesterday),
        dateEnd: toLocalDate(yesterday),
        type: 'daily',
        startTs: startOfDay(yesterday),
        endTs: endOfDay(yesterday),
      },
      {
        label: 'This Week',
        dateStart: toLocalDate(thisMonday),
        dateEnd: toLocalDate(today),
        type: 'weekly',
        startTs: startOfDay(thisMonday),
        endTs: endOfDay(today),
      },
      {
        label: 'Last Week',
        dateStart: toLocalDate(lastMonday),
        dateEnd: toLocalDate(lastSunday),
        type: 'weekly',
        startTs: startOfDay(lastMonday),
        endTs: endOfDay(lastSunday),
      },
    ]

    // Smart default: which card to emphasize
    let suggestedIndex: number
    if (dayOfWeek === 1 && hour < 12) {
      suggestedIndex = 3 // Monday morning → Last Week
    } else if (hour < 12) {
      suggestedIndex = 1 // Other mornings → Yesterday
    } else {
      suggestedIndex = 0 // Afternoon/evening → Today
    }

    return { cards, suggestedIndex }
  }, [])
}
