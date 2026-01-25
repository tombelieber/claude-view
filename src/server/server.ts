import express from 'express'
import getPort from 'get-port'
import open from 'open'
import { fileURLToPath } from 'url'
import { dirname, join } from 'path'
import { getProjects } from './sessions.js'

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
