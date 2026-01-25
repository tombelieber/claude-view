import express from 'express'
import getPort from 'get-port'
import open from 'open'
import { fileURLToPath } from 'url'
import { dirname, join } from 'path'
import { homedir } from 'os'
import { getProjects } from './sessions.js'
import { parseSession } from './parser.js'

const __dirname = dirname(fileURLToPath(import.meta.url))

export async function startServer(): Promise<number> {
  const app = express()
  const port = await getPort({ port: [3000, 3001, 3002, 3003, 3004, 3005] })

  // API routes
  app.get('/api/projects', async (_req, res) => {
    try {
      const projects = await getProjects()
      res.json(projects)
    } catch (error) {
      console.error('Failed to get projects:', error)
      res.status(500).json({ error: 'Failed to retrieve projects' })
    }
  })

  app.get('/api/session/:projectDir/:sessionId', async (req, res) => {
    try {
      const { projectDir, sessionId } = req.params

      // Construct the file path: ~/.claude/projects/{projectDir}/{sessionId}.jsonl
      const filePath = join(homedir(), '.claude', 'projects', projectDir, `${sessionId}.jsonl`)

      const session = await parseSession(filePath)
      res.json(session)
    } catch (error) {
      if (error instanceof Error && error.message.includes('not found')) {
        res.status(404).json({ error: 'Session not found' })
      } else {
        console.error('Failed to parse session:', error)
        res.status(500).json({ error: 'Failed to parse session' })
      }
    }
  })

  // Serve static frontend in production
  const clientPath = join(__dirname, '..', 'client')
  app.use(express.static(clientPath))
  app.get('*', (_req, res) => {
    res.sendFile(join(clientPath, 'index.html'))
  })

  return new Promise((resolve) => {
    app.listen(port, () => {
      open(`http://localhost:${port}`)
      resolve(port)
    })
  })
}
