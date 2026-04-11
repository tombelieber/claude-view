import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'happy-dom',
    include: [
      'src/**/*.test.ts',
      'src/components/conversation/blocks/shared/__tests__/SessionInteractionCard.test.tsx',
      'src/components/conversation/blocks/shared/__tests__/CollapsibleJson.test.tsx',
      'src/components/conversation/blocks/shared/__tests__/StatusBadge.test.tsx',
    ],
    setupFiles: ['./src/test-setup.ts'],
    passWithNoTests: true,
  },
})
