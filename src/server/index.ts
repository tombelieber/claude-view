#!/usr/bin/env node
import { startServer } from './server.js'

async function main() {
  const port = await startServer()
  console.log(`\n  Claude View running at http://localhost:${port}\n`)
}

main().catch((error) => {
  console.error('Failed to start server:', error)
  process.exit(1)
})
