import { useMemo } from 'react'

interface CardConfig {
  label: string
  dateStart: string
  dateEnd: string
  type: 'daily' | 'weekly'
  startTs: number
  endTs: number
}

interface SmartDefaults {
  primary: CardConfig
  secondary: CardConfig
}

/** Get YYYY-MM-DD for a Date in local time. */
function toLocalDate(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** Get start-of-day unix timestamp for a Date. */
function startOfDay(d: Date): number {
  const copy = new Date(d)
  copy.setHours(0, 0, 0, 0)
  return Math.floor(copy.getTime() / 1000)
}

/** Get end-of-day unix timestamp for a Date. */
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
    const dayOfWeek = now.getDay() // 0=Sun, 1=Mon

    const today = new Date(now)
    today.setHours(0, 0, 0, 0)

    const yesterday = new Date(today)
    yesterday.setDate(yesterday.getDate() - 1)

    const thisMonday = getMonday(today)
    const lastMonday = new Date(thisMonday)
    lastMonday.setDate(lastMonday.getDate() - 7)
    const lastSunday = new Date(thisMonday)
    lastSunday.setDate(lastSunday.getDate() - 1)

    const todayConfig: CardConfig = {
      label: 'Today',
      dateStart: toLocalDate(today),
      dateEnd: toLocalDate(today),
      type: 'daily',
      startTs: startOfDay(today),
      endTs: endOfDay(today),
    }

    const yesterdayConfig: CardConfig = {
      label: 'Yesterday',
      dateStart: toLocalDate(yesterday),
      dateEnd: toLocalDate(yesterday),
      type: 'daily',
      startTs: startOfDay(yesterday),
      endTs: endOfDay(yesterday),
    }

    const thisWeekConfig: CardConfig = {
      label: 'This Week',
      dateStart: toLocalDate(thisMonday),
      dateEnd: toLocalDate(today),
      type: 'weekly',
      startTs: startOfDay(thisMonday),
      endTs: endOfDay(today),
    }

    const lastWeekConfig: CardConfig = {
      label: 'Last Week',
      dateStart: toLocalDate(lastMonday),
      dateEnd: toLocalDate(lastSunday),
      type: 'weekly',
      startTs: startOfDay(lastMonday),
      endTs: endOfDay(lastSunday),
    }

    // Morning (before noon) -- show yesterday as primary
    if (hour < 12) {
      // Monday morning -- show last week
      if (dayOfWeek === 1) {
        return { primary: lastWeekConfig, secondary: todayConfig }
      }
      return { primary: yesterdayConfig, secondary: todayConfig }
    }

    // Afternoon/evening -- show today as primary
    return { primary: todayConfig, secondary: thisWeekConfig }
  }, [])
}
