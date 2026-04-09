/**
 * Mock Ink prompt that mimics Claude Code's AskUserQuestion rendering.
 *
 * Reads options from MOCK_OPTIONS env var (JSON array of {label, description}).
 * Writes the selected option's label to RESULT_FILE on selection.
 * Writes the full options array to OPTIONS_FILE so the test can verify ordering.
 *
 * Usage:
 *   RESULT_FILE=/tmp/result.txt \
 *   OPTIONS_FILE=/tmp/options.json \
 *   MOCK_OPTIONS='[{"label":"Option A","description":"First"},{"label":"Option B","description":"Second"},{"label":"Option C","description":"Third"}]' \
 *   node dist/index.js
 */
import { render, Text, Box } from 'ink'
import SelectInput from 'ink-select-input'
import { writeFileSync } from 'node:fs'

interface Option {
  label: string
  description: string
}

const resultFile = process.env.RESULT_FILE
const optionsFile = process.env.OPTIONS_FILE
const mockOptionsRaw = process.env.MOCK_OPTIONS

if (!resultFile || !mockOptionsRaw) {
  console.error('RESULT_FILE and MOCK_OPTIONS env vars required')
  process.exit(1)
}

const options: Option[] = JSON.parse(mockOptionsRaw)

// Write the options array so the test script can read what was rendered
if (optionsFile) {
  writeFileSync(optionsFile, JSON.stringify(options, null, 2))
}

const items = options.map((opt) => ({
  label: opt.label,
  value: opt.label,
}))

function App() {
  return (
    <Box flexDirection="column">
      <Text bold color="yellow">
        ? Which approach should I take?
      </Text>
      <Text dimColor> (Use arrow keys to navigate, Enter to select)</Text>
      <Box marginTop={1}>
        <SelectInput
          items={items}
          onSelect={(item) => {
            writeFileSync(resultFile!, item.value)
            // Small delay so the terminal renders the selection before exit
            setTimeout(() => process.exit(0), 100)
          }}
        />
      </Box>
    </Box>
  )
}

render(<App />)
