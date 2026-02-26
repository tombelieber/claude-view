/**
 * Shared theme tokens. These inline values are the initial placeholder —
 * Task 7 Step 8 replaces this file with re-exports from @claude-view/design-tokens.
 * IMPORTANT: These shapes must match design-tokens exports exactly (arrays for
 * fontFamily, numeric keys for spacing) to avoid breaking changes at replacement.
 */
export const colors = {
  primary: {
    50: '#eff6ff',
    100: '#dbeafe',
    200: '#bfdbfe',
    300: '#93c5fd',
    400: '#60a5fa',
    500: '#3b82f6',
    600: '#2563eb',
    700: '#1d4ed8',
    800: '#1e40af',
    900: '#1e3a8a',
  },
  status: {
    active: '#22c55e',
    waiting: '#f59e0b',
    idle: '#3b82f6',
    done: '#6b7280',
    error: '#ef4444',
  },
} as const;

export const spacing = {
  0: 0,
  px: 1,
  0.5: 2,
  1: 4,
  2: 8,
  3: 12,
  4: 16,
  5: 20,
  6: 24,
  8: 32,
  10: 40,
  12: 48,
  16: 64,
} as const;

export const fontFamily = {
  sans: ['Fira Sans', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'sans-serif'],
  mono: ['Fira Code', 'ui-monospace', 'SFMono-Regular', 'SF Mono', 'Menlo', 'Consolas', 'monospace'],
} as const;

export const fontSize = {
  xs: 12,
  sm: 14,
  base: 16,
  lg: 18,
  xl: 20,
  '2xl': 24,
  '3xl': 30,
} as const;
