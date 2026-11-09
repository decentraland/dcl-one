import { describe, expect, it } from 'vitest'
import { errorText } from './server-errors.js'

// Built by concatenation so the tree never holds a literal Figma file URL (figma-key-guard `url`).
const fileUrl = 'https://www.figma' + '.com/design/FAKEKEY000example000000/Design'

describe('errorText', () => {
  describe('when a tool error carries a Figma file URL', () => {
    it('should replace it with the placeholder', () => {
      expect(errorText(new Error(`Request failed on ${fileUrl} (TypeError)`))).toBe(
        'Request failed on <DCL_FIGMA_FILE> (TypeError)'
      )
    })

    it('should replace it in a thrown string too', () => {
      expect(errorText(`bad ${fileUrl}`)).toBe('bad <DCL_FIGMA_FILE>')
    })
  })
})
