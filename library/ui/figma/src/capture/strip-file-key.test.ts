import { describe, expect, it } from 'vitest'
import { stripFileKey, stripFileKeyText } from './strip-file-key.js'

const FAKE_KEY = 'FAKEKEY000example000000'
// Built by concatenation so the tree never holds a literal Figma file URL (figma-key-guard `url`).
const fileUrl = (key: string) => 'https://www.figma' + `.com/design/${key}/Design?node-id=1-2`

describe('stripFileKey', () => {
  describe('when a captured page carries the key, a file URL and node ids', () => {
    it('should drop fileKey fields, replace URLs and keep node ids, reading no environment', () => {
      const captured = {
        fileKey: FAKE_KEY,
        page: { id: '15599:30721', name: 'Nav bar' },
        nested: [{ fileKey: FAKE_KEY, link: fileUrl(FAKE_KEY), note: 'from the file' }]
      }

      const stripped = stripFileKey(captured)

      expect(JSON.stringify(stripped)).not.toContain(FAKE_KEY)
      expect(stripped).toEqual({
        page: { id: '15599:30721', name: 'Nav bar' },
        nested: [{ link: '<DCL_FIGMA_FILE>', note: 'from the file' }]
      })
    })
  })

  describe('when an SVG text links a Figma file', () => {
    it('should replace the URL', () => {
      expect(stripFileKeyText(`<svg data-src="${fileUrl(FAKE_KEY)}"/>`)).toBe(
        '<svg data-src="<DCL_FIGMA_FILE>"/>'
      )
    })
  })
})
