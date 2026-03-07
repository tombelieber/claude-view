// sidecar/src/health.ts
import { Hono } from 'hono'
import type { HealthResponse } from './types.js'

const startTime = Date.now()

export function healthRouter(getActiveCount: () => number) {
  const router = new Hono()

  router.get('/', (c) => {
    const response: HealthResponse = {
      status: 'ok',
      activeSessions: getActiveCount(),
      uptime: Math.floor((Date.now() - startTime) / 1000),
    }
    return c.json(response)
  })

  return router
}
