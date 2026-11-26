import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { libraryComponentEntries, removeLibraryEntries } from './library.js'

describe('when reading public and internal component registry entries', () => {
  it('should retain visibility and order for both lists', () => {
    expect(libraryComponentEntries({ components: ['Button'], internal: ['NavBar'] })).toEqual([
      { name: 'Button', visibility: 'public' },
      { name: 'NavBar', visibility: 'internal' }
    ])
  })
  it('should accept a legacy registry without an internal list', () => {
    expect(libraryComponentEntries({ components: [] })).toEqual([])
  })
  describe.each([
    null,
    [],
    { components: null },
    { internal: null },
    { components: 'Button' },
    { internal: 'NavBar' },
    { components: ['A', 'A'] },
    { components: ['A'], internal: ['A'] },
    { internal: ['../NavBar'] },
    { internal: [false] }
  ])('and the registry is malformed: %j', (value) => {
    let registry: unknown
    beforeEach(() => {
      registry = value
    })
    it('should reject rather than silently omit malformed or duplicate entries', () => {
      expect(() => libraryComponentEntries(registry)).toThrow(/library component registry/)
    })
  })
})

describe.each(['../runtime.js', './aggregates.js'])(
  'when %s initializes before ownership',
  (entry) => {
    let guarded: readonly string[]
    beforeEach(async () => {
      vi.resetModules()
      await import(entry)
      guarded = (await import('./ownership.js')).GUARDED_FILES
    })
    it('should initialize every guarded path without circular-import placeholders', () => {
      expect([...guarded].every((file) => typeof file === 'string' && file.length > 0)).toBe(true)
      expect(guarded).toContain('components/promotion.json')
    })
  }
)

describe.each([
  { registry: '{invalid JSON', error: SyntaxError },
  {
    registry: JSON.stringify({ components: ['Button'], internal: 'NavBar' }),
    error: 'Invalid library component registry name list'
  },
  {
    registry: JSON.stringify({ components: ['Button'], internal: ['Button'] }),
    error: 'Invalid library component registry: Duplicate component registry name: Button'
  },
  {
    registry: JSON.stringify({ components: [{ name: 'Button', limits: [] }] }),
    error: 'Invalid library component registry'
  },
  {
    registry: JSON.stringify({
      components: [{ name: 'Button', limits: [{ id: 'gap', text: 'See https://x.test' }] }]
    }),
    error: 'Invalid library component registry'
  }
])('when retirement encounters malformed registry $registry', ({ registry, error }) => {
  let root: string
  let index: string
  let owned: string[]
  beforeEach(async () => {
    root = await mkdtemp(path.join(tmpdir(), 'dcl-library-retirement-'))
    index = "export { Button } from './components/Button/Button'\n"
    owned = ['index.ts#value:Button', 'components/promotion.json#component:Button']
    await mkdir(path.join(root, 'components'))
    await writeFile(path.join(root, 'index.ts'), index)
    await writeFile(path.join(root, 'components/promotion.json'), registry)
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should reject before modifying either the index or registry', async () => {
    await expect(removeLibraryEntries(root, owned)).rejects.toThrow(error)
    expect(await readFile(path.join(root, 'index.ts'), 'utf8')).toBe(index)
    expect(await readFile(path.join(root, 'components/promotion.json'), 'utf8')).toBe(registry)
  })
})

describe('when retirement removes promotion entries', () => {
  const limits = [{ id: 'logo-size', text: 'The logo size is pending.' }]
  let root: string
  const read = async () =>
    JSON.parse(await readFile(path.join(root, 'components/promotion.json'), 'utf8'))
  beforeEach(async () => {
    root = await mkdtemp(path.join(tmpdir(), 'dcl-library-retirement-'))
    await mkdir(path.join(root, 'components'))
    await writeFile(
      path.join(root, 'components/promotion.json'),
      JSON.stringify({
        schema: 1,
        components: ['Button', { name: 'NavBar', limits }, { name: 'Card', limits }],
        internal: ['Icon']
      })
    )
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  describe('and the retired entry carries accepted limits', () => {
    it('should remove that entry and keep the other entries with their limits', async () => {
      expect(
        await removeLibraryEntries(root, ['components/promotion.json#component:NavBar'])
      ).toEqual(['components/promotion.json#component:NavBar'])
      expect(await read()).toEqual({
        schema: 1,
        components: ['Button', { name: 'Card', limits }],
        internal: ['Icon']
      })
    })
  })
  describe('and the retired entries are bare names', () => {
    it('should remove the public and the internal names', async () => {
      await removeLibraryEntries(root, [
        'components/promotion.json#component:Button',
        'components/promotion.json#component:Icon'
      ])
      expect(await read()).toEqual({
        schema: 1,
        components: [
          { name: 'NavBar', limits },
          { name: 'Card', limits }
        ],
        internal: []
      })
    })
  })
})
