import express from 'express'
import getPort from 'get-port'
import open from 'open'
import { fileURLToPath } from 'url'
import { dirname, join } from 'path'

const __dirname = dirname(fileURLToPath(import.meta.url))

export async function startServer(): Promise<number> {
  const app = express()
  const port = await getPort({ port: [3000, 3001, 3002, 3003, 3004, 3005] })

  // API routes (to be implemented)
  app.get('/api/projects', (_req, res) => {
    res.json([])
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
