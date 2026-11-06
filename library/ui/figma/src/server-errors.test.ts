import { afterEach, describe, expect, it, vi } from 'vitest'
import { errorText } from './server-errors.js'

const KEY = 'FAKEKEY000example000000'

describe('errorText', () => {
  afterEach(() => vi.unstubAllEnvs())

  describe('when a tool error names the configured file key', () => {
    it('should redact the key from the text the MCP response carries', () => {
      vi.stubEnv('FIGMA_FILE_KEY', KEY)

      const text = errorText(new Error(`Request failed on /v1/files/${KEY}/nodes: TypeError`))

      expect(text).not.toContain(KEY)
      expect(text).toBe('Request failed on /v1/files/<DCL_FIGMA_FILE>/nodes: TypeError')
    })

    it('should redact a thrown string too', () => {
      vi.stubEnv('FIGMA_FILE_KEY', KEY)

      expect(errorText(`bad ${KEY}`)).toBe('bad <DCL_FIGMA_FILE>')
    })
  })
})
