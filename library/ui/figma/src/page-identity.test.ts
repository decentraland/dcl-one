import { beforeEach, describe, expect, it } from 'vitest'
import { configSchema, type FigmaConfig } from './config.js'
import { resolvePageIdentities, selectCapturePages, mergeSyncState } from './page-identity.js'

describe('when resolving configured page identities', () => {
  let config: FigmaConfig
  let pages: { id: string; name: string; ready: boolean | null }[]
  beforeEach(() => {
    config = configSchema.parse({
      outDir: 'out',
      pages: {
        '1:1': { id: '1:1', name: 'Navigation', kind: 'components' },
        '1:2': { id: '1:2', name: 'Frozen', kind: 'components' },
        'legacy:Theme': { id: null, name: 'Theme', kind: 'tokens' }
      }
    })
    pages = [
      { id: '1:1', name: 'Navigation ✅', ready: true },
      { id: '1:2', name: 'Frozen', ready: false }
    ]
  })
  it('should resolve a renamed ready page by ID and preserve its output name', () => {
    expect(resolvePageIdentities(config, pages).find((page) => page.id === '1:1')).toMatchObject({
      name: 'Navigation',
      liveName: 'Navigation ✅',
      renamed: true
    })
  })
  it('should accept an exact ID or unique live name for a single-page request', () => {
    expect(
      selectCapturePages(resolvePageIdentities(config, pages), 'Navigation ✅').map(
        (page) => page.id
      )
    ).toEqual(['1:1'])
    expect(
      selectCapturePages(resolvePageIdentities(config, pages), '1:1').map((page) => page.id)
    ).toEqual(['1:1'])
  })
  it('should retain unresolved legacy entries without capturing them', () => {
    expect(resolvePageIdentities(config, pages).find((page) => page.id === null)).toMatchObject({
      name: 'Theme',
      legacy: true
    })
  })
  describe('and a configured page is missing', () => {
    beforeEach(() => {
      pages[0]!.id = 'new-id'
    })
    it('should fail before capture', () => {
      expect(() => resolvePageIdentities(config, pages)).toThrow('Missing configured Figma page')
    })
  })
  describe('and a known ID takes another page display name', () => {
    it('should keep resolving exclusively by ID', () => {
      expect(resolvePageIdentities(config, pages)[0]).toMatchObject({ id: '1:1', renamed: true })
    })
  })
  describe('and two live pages have the same requested name', () => {
    beforeEach(() => {
      pages[1]!.name = 'Navigation ✅'
    })
    it('should reject the ambiguous request', () => {
      expect(() =>
        selectCapturePages(resolvePageIdentities(config, pages), 'Navigation ✅')
      ).toThrow('Ambiguous Figma page')
    })
  })
  describe('and only one page is selected', () => {
    it('should preserve every unrelated readiness entry', () => {
      expect(
        mergeSyncState(
          [
            { id: '1:1', name: 'Navigation', ready: false },
            { id: '1:2', name: 'Frozen old', ready: true }
          ],
          pages,
          ['1:1']
        )
      ).toEqual([pages[0], { id: '1:2', name: 'Frozen old', ready: true }])
    })
  })
  describe('and known IDs exchange names', () => {
    beforeEach(() => {
      pages[0]!.name = 'Frozen'
      pages[1]!.name = 'Navigation'
    })
    it('should report both renames without changing identity', () => {
      expect(
        resolvePageIdentities(config, pages)
          .filter((page) => page.renamed)
          .map((page) => page.id)
      ).toEqual(['1:1', '1:2'])
    })
  })
  describe('and known IDs share a display name', () => {
    beforeEach(() => {
      pages[1]!.name = pages[0]!.name
    })
    it('should accept an explicit ID without considering display-name ambiguity', () => {
      expect(
        selectCapturePages(resolvePageIdentities(config, pages), '1:1').map((page) => page.id)
      ).toEqual(['1:1'])
    })
  })
  describe('and duplicate display names happen to equal a configured page ID', () => {
    beforeEach(() => {
      pages[0]!.name = '1:1'
      pages[1]!.name = '1:1'
    })
    it('should give the explicit ID priority over name-based lookup', () => {
      expect(
        selectCapturePages(resolvePageIdentities(config, pages), '1:1').map((page) => page.id)
      ).toEqual(['1:1'])
    })
  })
})
