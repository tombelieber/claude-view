// sidecar/src/health.ts
import { Hono } from 'hono'
import type { HealthResponse } from './protocol.js'

const startTime = Date.now()

export function healthRouter(getActiveCount: () => number) {
  const router = new Hono()

  router.get('/', (c) => {
    const mem = process.memoryUsage()
    const response: HealthResponse & { memory?: Record<string, string> } = {
      status: 'ok',
      activeSessions: getActiveCount(),
      uptime: Math.floor((Date.now() - startTime) / 1000),
      memory: {
        rss: `${(mem.rss / 1024 / 1024).toFixed(0)} MB`,
        heapUsed: `${(mem.heapUsed / 1024 / 1024).toFixed(0)} MB`,
        heapTotal: `${(mem.heapTotal / 1024 / 1024).toFixed(0)} MB`,
        external: `${(mem.external / 1024 / 1024).toFixed(0)} MB`,
        arrayBuffers: `${(mem.arrayBuffers / 1024 / 1024).toFixed(0)} MB`,
      },
    }
    return c.json(response)
  })

  return router
}
