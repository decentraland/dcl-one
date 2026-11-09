import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  CACHED_LEDGERS,
  LEDGER,
  RETIREMENTS,
  findConsumers,
  readTree,
  type Tree
} from './consumers.js'

const REVIEW = 'figma/review/src/generated/components/assets/0123456789abcdef.svg'
const LIBRARY = 'components/assets/0123456789abcdef.svg'
const NAME = '0123456789abcdef.svg'

const tree = (texts: Record<string, string>): Tree => ({
  paths: new Set([REVIEW, LIBRARY, ...Object.keys(texts)]),
  texts: new Map(Object.entries(texts))
})
const users = (texts: Record<string, string>) =>
  findConsumers(tree(texts), REVIEW).map((item) => item.file)

describe('the lifecycle ledgers', () => {
  // The snapshot folder is the gitignored fetch-source cache: a ledger there is never committed and a re-fetch
  // replaces it, so readers and writers use the committed copies that fetch-source overlays onto the cache.
  it('should live outside the gitignored capture cache, in the committed figma/ledger', () => {
    const libraryRoot = path.resolve(import.meta.dirname, '../../..')
    for (const file of [LEDGER, RETIREMENTS]) {
      expect(file.startsWith('figma/ledger/')).toBe(true)
      expect(() =>
        execFileSync('git', ['-C', libraryRoot, 'check-ignore', '-q', file], { stdio: 'pipe' })
      ).toThrow()
    }
    for (const file of CACHED_LEDGERS)
      expect(
        execFileSync('git', ['-C', libraryRoot, 'check-ignore', file], { stdio: 'pipe' })
          .toString()
          .trim()
      ).toBe(file)
  })

  it("should not count the cache's copies of the ledgers as consumers", () => {
    expect(
      users({
        [CACHED_LEDGERS[0]!]: JSON.stringify({ orphaned: [{ path: REVIEW }] }),
        [CACHED_LEDGERS[1]!]: JSON.stringify({ owned: [REVIEW] })
      })
    ).toEqual([])
  })
})

describe('findConsumers', () => {
  describe('when a reference resolves to a path', () => {
    it('should not count a library CSS URL that resolves to its own copy', () => {
      expect(
        users({
          'components/places/PlaceCard.module.css': `a { background: url("../assets/${NAME}") }`
        })
      ).toEqual([])
    })

    it('should count a foreign JSX import that resolves to the Review asset', () => {
      expect(
        users({
          'figma/review/src/Hero.jsx': `import art from './generated/components/assets/${NAME}'`
        })
      ).toEqual(['figma/review/src/Hero.jsx'])
    })

    it('should count a consumer in another root that resolves to the Review asset', () => {
      expect(
        users({
          'scripts/use.mjs': `readFile('../figma/review/src/generated/components/assets/${NAME}')`
        })
      ).toEqual(['scripts/use.mjs'])
    })

    it('should count a repository path written in a document', () => {
      expect(users({ 'docs/assets.md': `See ${REVIEW}.` })).toEqual(['docs/assets.md'])
    })
  })

  describe('when a reference cannot be resolved', () => {
    it('should count a bare basename in markdown', () => {
      expect(users({ 'docs/note.md': `The ${NAME} artwork.` })).toEqual(['docs/note.md'])
    })

    it('should count a bare hash without an extension', () => {
      expect(users({ 'docs/note.md': 'hash 0123456789abcdef' })).toEqual(['docs/note.md'])
    })

    it('should count an unresolved basename even when both copies exist', () => {
      expect(
        users({
          'components/places/Card.tsx': `const icon = '${NAME}'`
        })
      ).toEqual(['components/places/Card.tsx'])
    })

    it('should count a package specifier in code', () => {
      expect(users({ 'figma/review/src/x.ts': `import a from 'pkg/assets/${NAME}'` })).toEqual([
        'figma/review/src/x.ts'
      ])
    })
  })

  describe('when one file has a resolved reference and another one', () => {
    it.each([
      ['an unresolved basename', `url(${NAME})`],
      [
        'a reference to the Review asset',
        `url(../../figma/review/src/generated/components/assets/${NAME})`
      ]
    ])('should still count it for %s', (_, second) => {
      expect(
        users({
          'components/places/PlaceCard.module.css': `a { background: url(../assets/${NAME}) } b { background: ${second} }`
        })
      ).toEqual(['components/places/PlaceCard.module.css'])
    })
  })

  describe('when bookkeeping lists the asset', () => {
    it('should ignore the ledger, retirements and the files keys of integrity manifests', () => {
      expect(
        users({
          'figma/ledger/orphaned-assets.json': JSON.stringify({
            orphaned: [{ path: REVIEW }]
          }),
          'figma/ledger/retirements.json': JSON.stringify({ owned: [REVIEW] }),
          'figma/review/src/review/promotion.json': JSON.stringify({
            files: { [REVIEW]: 'f'.repeat(64) }
          }),
          'components/promotion.json': JSON.stringify({
            files: { [LIBRARY]: 'f'.repeat(64) }
          })
        })
      ).toEqual([])
    })

    it('should ignore the fingerprint of the asset itself, which starts with its name', () => {
      expect(
        users({
          'figma/review/src/review/promotion.json': JSON.stringify({
            files: { [REVIEW]: `0123456789abcdef${'0'.repeat(48)}` }
          })
        })
      ).toEqual([])
    })

    it('should count a files key whose value is not a fingerprint', () => {
      expect(
        users({
          'figma/review/src/review/promotion.json': JSON.stringify({
            files: { [REVIEW]: 'pending' }
          })
        })
      ).toEqual(['figma/review/src/review/promotion.json'])
    })

    it('should count a files value that is not a fingerprint', () => {
      expect(
        users({
          'figma/review/src/review/promotion.json': JSON.stringify({ files: { other: REVIEW } })
        })
      ).toEqual(['figma/review/src/review/promotion.json'])
    })

    it('should count the same path anywhere else in an integrity manifest', () => {
      expect(
        users({
          'figma/review/src/review/promotion.json': JSON.stringify({
            files: { [REVIEW]: 'f'.repeat(64) },
            note: REVIEW
          })
        })
      ).toEqual(['figma/review/src/review/promotion.json'])
    })
  })
})

describe('readTree in a git checkout', () => {
  it('should leave out a tracked file Git reports as deleted', async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), 'consumers-git-'))
    try {
      await writeFile(path.join(root, 'kept.md'), 'kept')
      await writeFile(path.join(root, 'gone.md'), 'gone')
      execFileSync('git', ['init', '-q'], { cwd: root })
      execFileSync('git', ['add', '-A'], { cwd: root })
      await rm(path.join(root, 'gone.md'))
      expect([...(await readTree(root)).texts.keys()]).toEqual(['kept.md'])
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('should fail closed on a listed file it cannot read', async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), 'consumers-git-'))
    try {
      await mkdir(path.join(root, 'odd.md'))
      await writeFile(path.join(root, 'odd.md', 'inner.txt'), 'x')
      await writeFile(path.join(root, 'plain.md'), 'x')
      execFileSync('git', ['init', '-q'], { cwd: root })
      execFileSync('git', ['add', '-A'], { cwd: root })
      await rm(path.join(root, 'plain.md'))
      await mkdir(path.join(root, 'plain.md'))
      await expect(readTree(root)).rejects.toThrow('plain.md')
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
