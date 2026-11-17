import { describe, expect, it } from 'vitest'
import { FigmaClient } from './figma/client.js'
import { probeAccess } from './probe.js'

const fileResponse = { document: { children: [{ name: ' Colors' }, { name: 'Icons' }] } }

function fakeFetch(variablesStatus: number): typeof fetch {
  return (async (url: string) => {
    if (String(url).includes('/variables/local')) {
      return new Response('{}', { status: variablesStatus })
    }
    return new Response(JSON.stringify(fileResponse), { status: 200 })
  }) as unknown as typeof fetch
}

const quiet = { sleep: async () => {}, log: () => {} }

describe('probeAccess', () => {
  describe('when the token can read the file and its variables', () => {
    it('should report both as ok and list the trimmed page names', async () => {
      const result = await probeAccess(
        new FigmaClient({ getToken: async () => 'token' }, fakeFetch(200)),
        'a'.repeat(22)
      )

      expect(result).toEqual({ file: 'ok', variables: 'ok', pages: ['Colors', 'Icons'] })
    })
  })

  describe('when the variables endpoint is forbidden', () => {
    it('should report variables as forbidden and keep the file readable', async () => {
      const result = await probeAccess(
        new FigmaClient({ getToken: async () => 'token' }, fakeFetch(403)),
        'a'.repeat(22)
      )

      expect(result.file).toBe('ok')
      expect(result.variables).toBe('forbidden')
    })
  })

  describe('when the variables endpoint fails with an unexpected status', () => {
    it('should report variables as error', async () => {
      const result = await probeAccess(
        new FigmaClient({ getToken: async () => 'token' }, fakeFetch(500), quiet),
        'a'.repeat(22)
      )

      expect(result.variables).toBe('error')
    })
  })
})
