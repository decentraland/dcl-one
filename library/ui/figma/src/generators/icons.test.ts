import { describe, expect, it } from 'vitest'
import { snapshotSource } from '../../test/snapshot.js'
import type { IconSpec } from '../ir/schemas.js'
import { generateIcons } from './icons.js'

const context = {}

const icon = (overrides: Partial<IconSpec>): IconSpec => ({
  name: 'general/like',
  nodeId: '1:1',
  section: 'General',
  category: 'general',
  group: 'general',
  variant: null,
  width: 24,
  height: 24,
  file: 'x.svg',
  svg: '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M1 1h10v10H1z" stroke="#FCFCFC" stroke-width="2"/></svg>',
  ...overrides
})

const contentOf = (files: { path: string; content: string }[], file: string) =>
  files.find((candidate) => candidate.path === file)!.content

describe('generateIcons', () => {
  describe('when an icon uses a single color', () => {
    it('should render it with currentColor and flag it monochrome', () => {
      const { files, meta } = generateIcons([icon({})], context)

      expect(contentOf(files, 'icons/general.tsx')).toContain('stroke="currentColor"')
      expect(meta[0]).toMatchObject({ component: 'GeneralLike', monochrome: true })
    })
  })

  describe('when an icon mixes colors', () => {
    it('should keep the original colors', () => {
      const svg =
        '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M1 1h10v10H1z" fill="#FF2D55"/><path d="M12 12h10v10H12z" fill="#FCFCFC"/></svg>'
      const { files, meta } = generateIcons([icon({ svg })], context)

      expect(contentOf(files, 'icons/general.tsx')).toContain('#ff2d55')
      expect(meta[0]?.monochrome).toBe(false)
    })
  })

  describe('when two figma names resolve to the same identifier', () => {
    it('should generate distinct component names', () => {
      const { meta } = generateIcons(
        [icon({ name: 'general/reset-2' }), icon({ name: 'general/reset2', nodeId: '1:2' })],
        context
      )

      expect(meta.map((m) => m.component)).toEqual(['GeneralReset2', 'GeneralReset2_2'])
    })
  })

  describe('when a navbar icon has active=false and active=true variants', () => {
    it('should generate one component with an active prop', () => {
      const { files, meta } = generateIcons(
        [
          icon({ name: 'navbar/wallet', category: 'navbar', variant: 'active=false' }),
          icon({ name: 'navbar/wallet', category: 'navbar', variant: 'active=true', nodeId: '1:2' })
        ],
        context
      )

      expect(contentOf(files, 'icons/navbar.tsx')).toContain('active = false')
      expect(meta).toHaveLength(1)
      expect(meta[0]?.hasActiveState).toBe(true)
    })
  })

  describe('when an icon has a variant the generator does not understand', () => {
    it('should report it without emitting a guessed replacement', () => {
      const output = generateIcons([icon({ variant: 'size=large' })], context)
      expect(output.meta).toEqual([])
      expect(output.skipped).toEqual([
        { name: 'general/like', reason: 'unsupported or ambiguous variants: 1:1 (size=large)' }
      ])
    })
  })

  describe('when an icon embeds a raster image', () => {
    it('should skip it and report why', () => {
      const svg = '<svg viewBox="0 0 24 24"><image href="data:image/png;base64,AAAA"/></svg>'
      const { meta, skipped } = generateIcons([icon({ svg })], context)

      expect(meta).toHaveLength(0)
      expect(skipped).toEqual([{ name: 'general/like', reason: 'embeds a raster image' }])
    })
  })

  describe('when generating the real snapshot', () => {
    it('should retain supported icons and report ambiguous or raster-only source masters', async () => {
      const { meta, skipped } = generateIcons(await snapshotSource().getIcons(), context)

      expect(meta).toHaveLength(474)
      expect(skipped).toEqual(
        expect.arrayContaining([expect.objectContaining({ name: 'map/avatar-pic' })])
      )
      expect(skipped.every((item) => item.reason.length > 0)).toBe(true)
    })

    it('should keep component names unique', async () => {
      const { meta } = generateIcons(await snapshotSource().getIcons(), context)

      expect(new Set(meta.map((m) => m.component)).size).toBe(meta.length)
    })
  })
})

describe('when a category cannot be a TypeScript import binding', () => {
  it.each(['social media', 'arrow-icons', 'default', '../outside'])(
    'should reject %j before generating paths',
    (category) => {
      expect(() => generateIcons([icon({ category })], context)).toThrow(
        'Invalid generated TypeScript identifier'
      )
    }
  )
})

describe('when one icon has an invalid component identifier', () => {
  it('should report it as skipped while retaining valid icons', () => {
    const result = generateIcons([icon({ name: '3d view' }), icon({})], context)
    expect(result.meta.map((entry) => entry.component)).toEqual(['GeneralLike'])
    expect(result.skipped).toContainEqual(
      expect.objectContaining({ name: '3d view', reason: expect.stringContaining('identifier') })
    )
  })
})
