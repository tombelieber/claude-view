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
      'src/components/conversation/blocks/shared/__tests__/ToolChip.test.tsx',
      'src/components/conversation/blocks/shared/__tests__/InteractionCardFields.test.tsx',
      'src/components/conversation/blocks/chat/system-variants/__tests__/system-variants.test.tsx',
      'src/components/conversation/blocks/chat/__tests__/TeamTranscriptBlock.test.tsx',
      'src/components/conversation/blocks/chat/__tests__/TurnBoundary.test.tsx',
      'src/components/conversation/blocks/chat/__tests__/NoticeBlock.test.tsx',
    ],
    setupFiles: ['./src/test-setup.ts'],
    passWithNoTests: true,
  },
})
