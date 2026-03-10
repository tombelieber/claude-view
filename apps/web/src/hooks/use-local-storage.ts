import { useCallback, useState } from 'react'

export function useLocalStorage<T>(key: string, initialValue: T): [T, (value: T) => void] {
  const [stored, setStored] = useState<T>(() => {
    try {
      const item = localStorage.getItem(key)
      return item ? JSON.parse(item) : initialValue
    } catch {
      return initialValue
    }
  })

  const setValue = useCallback(
    (value: T) => {
      setStored(value)
      try {
        localStorage.setItem(key, JSON.stringify(value))
      } catch {
        // localStorage full or unavailable — ignore
      }
    },
    [key],
  )

  return [stored, setValue]
}
