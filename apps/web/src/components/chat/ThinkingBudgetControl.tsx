import { useCallback } from 'react'

const PRESETS = [
  { label: 'Default', value: null },
  { label: '1K', value: 1024 },
  { label: '4K', value: 4096 },
  { label: '16K', value: 16384 },
  { label: '64K', value: 65536 },
  { label: 'Max', value: 0 },
] as const

interface ThinkingBudgetControlProps {
  value: number | null
  onChange: (tokens: number | null) => void
  disabled?: boolean
}

export function ThinkingBudgetControl({ value, onChange, disabled }: ThinkingBudgetControlProps) {
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const v = e.target.value
      onChange(v === 'null' ? null : Number(v))
    },
    [onChange],
  )

  return (
    <select
      value={value === null ? 'null' : String(value)}
      onChange={handleChange}
      disabled={disabled}
      className="text-xs bg-transparent border border-border-secondary rounded px-1.5 py-0.5"
      title="Thinking budget"
    >
      {PRESETS.map((p) => (
        <option key={String(p.value)} value={p.value === null ? 'null' : String(p.value)}>
          {p.label}
        </option>
      ))}
    </select>
  )
}
