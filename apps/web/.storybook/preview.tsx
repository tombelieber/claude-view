import type { Preview } from '@storybook/react-vite'
import '../src/index.css'

/**
 * Theme decorator: reads the `theme` toolbar global and applies
 * Tailwind v4 dark mode class + matching background color.
 *
 * Works in both Canvas and Docs views — each story gets wrapped,
 * so the Docs page shows dark-themed stories inline.
 *
 * Our Tailwind config uses: @custom-variant dark (&:where(.dark, .dark *));
 * So we need .dark on an ancestor element — not media query.
 */
function ThemeDecorator(Story: React.ComponentType, context: { globals: Record<string, unknown> }) {
  const theme = (context.globals.theme as string) ?? 'light'
  const isDark = theme === 'dark'

  return (
    <div
      className={isDark ? 'dark' : ''}
      style={{
        backgroundColor: isDark ? '#0f0f17' : '#ffffff',
        color: isDark ? '#e5e5e5' : '#1a1a1a',
        minHeight: '100px',
        padding: '1rem',
        // In Docs view, each story sits inside the docs page —
        // round the corners so dark cards don't bleed into white docs bg
        borderRadius: '8px',
      }}
    >
      <Story />
    </div>
  )
}

const preview: Preview = {
  tags: ['autodocs'],
  parameters: {
    layout: 'centered',
    docs: {
      // Make the Docs page canvas background match the theme
      canvas: {
        sourceState: 'shown',
      },
    },
  },
  globalTypes: {
    theme: {
      description: 'Light / Dark mode',
      toolbar: {
        title: 'Theme',
        icon: 'circlehollow',
        items: [
          { value: 'light', title: 'Light', icon: 'sun' },
          { value: 'dark', title: 'Dark', icon: 'moon' },
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: {
    theme: 'light',
  },
  decorators: [ThemeDecorator as never],
}

export default preview
