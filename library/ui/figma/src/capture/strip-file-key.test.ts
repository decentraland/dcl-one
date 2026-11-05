import { describe, expect, it } from 'vitest'
import { stripFileKey, stripFileKeyText } from './strip-file-key.js'

const FAKE_KEY = 'FAKEKEY000example000000'
const env = { FIGMA_FILE_KEY: FAKE_KEY }
// Built by concatenation so the tree never holds a literal Figma file URL (figma-key-guard `url`).
const fileUrl = (key: string) => 'https://www.figma' + `.com/design/${key}/Design?node-id=1-2`

describe('stripFileKey', () => {
  describe('when a captured page carries the key, a file URL and node ids', () => {
    it('should drop fileKey fields, replace URLs and keep node ids', () => {
      const captured = {
        fileKey: FAKE_KEY,
        page: { id: '15599:30721', name: 'Nav bar' },
        nested: [{ fileKey: FAKE_KEY, link: fileUrl(FAKE_KEY), note: `from ${FAKE_KEY}` }]
      }

      const stripped = stripFileKey(captured, env)

      expect(JSON.stringify(stripped)).not.toContain(FAKE_KEY)
      expect(stripped).toEqual({
        page: { id: '15599:30721', name: 'Nav bar' },
        nested: [{ link: '<DCL_FIGMA_FILE>', note: 'from <DCL_FIGMA_FILE>' }]
      })
    })
  })

  describe('when the key is not configured', () => {
    it('should still remove fileKey fields and file URLs of any file', () => {
      const stripped = stripFileKey(
        { fileKey: 'OtherKey000000000000000', link: fileUrl('OtherKey0') },
        {}
      )

      expect(stripped).toEqual({ link: '<DCL_FIGMA_FILE>' })
    })
  })

  describe('when an SVG text embeds the key', () => {
    it('should redact it', () => {
      expect(stripFileKeyText(`<svg data-src="${FAKE_KEY}"/>`, env)).toBe(
        '<svg data-src="<DCL_FIGMA_FILE>"/>'
      )
    })
  })
})
