import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { compare, fdsSource, main, pinnedFiles } from './lock-digests.mjs'

const LOCK = 'library/ui/upstream.lock.json'
let repo: string
let out: string

const git = (...args: string[]) => execFileSync('git', ['-C', repo, ...args], { stdio: 'pipe' })
const put = async (file: string, content: string) => {
  await mkdir(path.dirname(path.join(repo, file)), { recursive: true })
  await writeFile(path.join(repo, file), content)
}
const lock = async () => JSON.parse(await readFile(path.join(repo, LOCK), 'utf8'))
const run = (...argv: string[]) => main(argv, repo)

beforeEach(async () => {
  repo = await mkdtemp(path.join(tmpdir(), 'lock-digests-'))
  git('init', '--quiet')
  await put('library/ui/figma/a.ts', 'export const a = 1\n')
  await put('library/ui/figma/src/b.ts', 'export const b = 2\n')
  await put('library/ui/README.md', 'outside the scope\n')
  await put(
    LOCK,
    JSON.stringify({
      sources: [{ repository: 'https://github.com/decentraland/figma-design-system' }]
    })
  )
  git('add', '-A')
  out = ''
  vi.spyOn(process.stdout, 'write').mockImplementation((chunk) => ((out += String(chunk)), true))
  vi.spyOn(process.stderr, 'write').mockImplementation((chunk) => ((out += String(chunk)), true))
})

afterEach(async () => {
  vi.restoreAllMocks()
  await rm(repo, { recursive: true, force: true })
})

// Gives every pinned file a counterpart-free dcl-one-only deviation with its reason.
type Lock = {
  sources: {
    revision?: string
    snapshot?: { revision?: string; digests?: Record<string, string> }
    deviationCodes?: Record<string, string | null>
    files: Record<string, { sha256: string; fds: unknown; deviation?: string; note?: string }>
  }[]
}
async function annotate(edit?: (value: Lock) => void) {
  const value = (await lock()) as Lock
  const source = value.sources[0]!
  source.revision = 'a'.repeat(40)
  source.snapshot = { revision: 'b'.repeat(40), digests: { 'pages.json': 'c'.repeat(64) } }
  source.deviationCodes = { 'dcl-one-only': 'These files are owned by this tree.', note: null }
  for (const entry of Object.values(source.files)) entry.deviation = 'dcl-one-only'
  edit?.(value)
  await put(LOCK, JSON.stringify(value))
  git('add', LOCK)
}

describe('when the lock pins every tracked figma file', () => {
  beforeEach(async () => {
    expect(run('--write')).toBe(0)
    await annotate()
  })

  it('should pin exactly the tracked files under library/ui/figma and pass', async () => {
    expect(Object.keys((await lock()).sources[0].files)).toEqual([
      'library/ui/figma/a.ts',
      'library/ui/figma/src/b.ts'
    ])
    expect(run()).toBe(0)
    expect(out).toContain('2 files match')
  })

  it('should fail on an edited file once it is staged', async () => {
    await put('library/ui/figma/a.ts', 'export const a = 3\n')
    git('add', 'library/ui/figma/a.ts')

    expect(run()).toBe(1)
    expect(out).toContain('CHANGED library/ui/figma/a.ts')
  })

  it('should fail on a pinned file that is no longer tracked', () => {
    git('rm', '--quiet', '--cached', '-f', 'library/ui/figma/src/b.ts')

    expect(run()).toBe(1)
    expect(out).toContain('MISSING library/ui/figma/src/b.ts')
  })

  it('should fail on a tracked file the lock does not pin', async () => {
    await put('library/ui/figma/extra.ts', 'export {}\n')
    git('add', 'library/ui/figma/extra.ts')

    expect(run()).toBe(1)
    expect(out).toContain('UNPINNED library/ui/figma/extra.ts')
  })

  it('should refuse a Git LFS pointer instead of pinning its text', async () => {
    await put(
      'library/ui/figma/font.woff2',
      'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'a'.repeat(64) + '\nsize 10\n'
    )
    git('add', 'library/ui/figma/font.woff2')

    expect(run()).toBe(1)
    expect(out).toContain('is a Git LFS pointer')
  })

  it('should keep counterparts and deviations when resealing', async () => {
    await put('library/ui/figma/a.ts', 'export const a = 9\n')
    git('add', 'library/ui/figma/a.ts')

    expect(run('--write')).toBe(0)
    const entry = (await lock()).sources[0].files['library/ui/figma/a.ts']
    expect(entry).toMatchObject({ fds: null, deviation: 'dcl-one-only' })
    git('add', LOCK)
    expect(run()).toBe(0)
  })

  it('should fail when a new file is pinned without a deviation', async () => {
    await put('library/ui/figma/new.ts', 'export {}\n')
    git('add', 'library/ui/figma/new.ts')
    expect(run('--write')).toBe(0)
    git('add', LOCK)

    expect(run()).toBe(1)
    expect(out).toContain(
      'DEVIATION library/ui/figma/new.ts differs from figma-design-system and names no deviation'
    )
  })

  it('should pass a file identical to its FDS blob with no deviation, and fail one that still names a code', async () => {
    await annotate((value) => {
      const entry = value.sources[0].files['library/ui/figma/a.ts']
      entry.fds = { path: 'dcl-mcp-figma/a.ts', sha256: entry.sha256 }
      delete entry.deviation
    })
    expect(run()).toBe(0)

    await annotate((value) => {
      const entry = value.sources[0].files['library/ui/figma/a.ts']
      entry.fds = { path: 'dcl-mcp-figma/a.ts', sha256: entry.sha256 }
    })
    expect(run()).toBe(1)
    expect(out).toContain('matches figma-design-system but still names a deviation')
  })

  it('should fail on an unknown code and on a code without a reason', async () => {
    await annotate((value) => {
      value.sources[0].files['library/ui/figma/a.ts'].deviation = 'made-up'
    })
    expect(run()).toBe(1)
    expect(out).toContain('names an unknown deviation code')

    out = ''
    await annotate((value) => {
      value.sources[0].deviationCodes['dcl-one-only'] = ' '
    })
    expect(run()).toBe(1)
    expect(out).toContain('deviation code dcl-one-only has no reason')

    out = ''
    await annotate((value) => {
      value.sources[0].deviationCodes = { 'dcl-one-only': 0 as unknown as string, note: null }
    })
    expect(run()).toBe(1)
    expect(out).toContain('deviation code dcl-one-only has no reason')
  })

  it('should fail when the note code carries a reason, since each file carries its own', async () => {
    await annotate((value) => {
      value.sources[0].deviationCodes!.note = 'shared'
    })
    expect(run()).toBe(1)
    expect(out).toContain('deviation code note must map to null')
  })

  it('should fail on unsorted codes and sort them on --write without changing a reason', async () => {
    await annotate((value) => {
      value.sources[0].deviationCodes = {
        note: null,
        'dcl-one-only': 'These files are owned by this tree.'
      }
    })
    expect(run()).toBe(1)
    expect(out).toContain('deviation codes are not in sorted order')

    expect(run('--write')).toBe(0)
    expect((await lock()).sources[0].deviationCodes).toEqual({
      'dcl-one-only': 'These files are owned by this tree.',
      note: null
    })
    expect(Object.keys((await lock()).sources[0].deviationCodes)).toEqual(['dcl-one-only', 'note'])
    git('add', LOCK)
    out = ''
    expect(run()).toBe(0)
  })

  it('should refuse an integer-like code id, which a JS object can never keep in string order', async () => {
    await annotate((value) => {
      value.sources[0].deviationCodes = {
        '10': 'Ten.',
        '9': 'Nine.',
        'dcl-one-only': 'These files are owned by this tree.',
        note: null
      }
    })
    expect(run()).toBe(1)
    expect(out).toContain('deviation code 9 is integer-like')
    expect(out).toContain('deviation code 10 is integer-like')
  })

  it('should refuse a tracked path with a tab, so a sibling cannot hide an edited file', async () => {
    await annotate()
    await put('library/ui/figma/a.ts\t.orig', 'export const a = 1\n')
    await put('library/ui/figma/a.ts', 'export const a = 9\n')
    git('add', '-A')
    expect(run()).toBe(1)
    expect(out).toContain('control character in its path')
  })

  it('should fail closed on a deviation code table that is not an object', async () => {
    await annotate((value) => {
      value.sources[0].deviationCodes = ['dcl-one-only'] as unknown as Record<string, string>
    })
    expect(run()).toBe(1)
    expect(out).toContain('malformed deviationCodes table')
  })

  describe.each(['constructor', 'toString', '__proto__'])(
    'and a file names inherited deviation %s',
    (code) => {
      let value: Lock

      beforeEach(async () => {
        await annotate((lock) => {
          lock.sources[0]!.files['library/ui/figma/a.ts']!.deviation = code
        })
        value = await lock()
      })

      it('should reject the code through the real pinned-file validator', () => {
        expect(pinnedFiles(value).problems).toContain(
          'library/ui/figma/a.ts names an unknown deviation code'
        )
      })
    }
  )

  describe('and constructor is explicitly declared as an own deviation code', () => {
    let value: Lock

    beforeEach(async () => {
      await annotate((lock) => {
        const source = lock.sources[0]!
        source.deviationCodes = { constructor: 'Declared on purpose.', ...source.deviationCodes }
        source.files['library/ui/figma/a.ts']!.deviation = 'constructor'
      })
      value = await lock()
    })

    it('should accept the declared code through the real validator', () => {
      expect(pinnedFiles(value).problems).toEqual([])
    })
  })

  it('should accept a deviation code declared as an own key with its reason', async () => {
    await annotate((lock) => {
      const source = lock.sources[0]!
      source.deviationCodes = {
        'dcl-one-only': source.deviationCodes!['dcl-one-only']!,
        hardened: 'This file is hardened here.',
        note: null
      }
      source.files['library/ui/figma/a.ts']!.deviation = 'hardened'
    })

    expect(pinnedFiles(await lock()).problems).toEqual([])
    expect(run()).toBe(0)
    expect(out).not.toContain('unknown deviation code')
  })

  it('should require a short plain note for the note code', async () => {
    await annotate((value) => {
      value.sources[0].files['library/ui/figma/a.ts'].deviation = 'note'
    })
    expect(run()).toBe(1)
    expect(out).toContain('needs a short plain note')

    await annotate((value) => {
      Object.assign(value.sources[0].files['library/ui/figma/a.ts'], {
        deviation: 'note',
        note: 'Kept from the previous release for one consumer.'
      })
    })
    expect(run()).toBe(0)
  })

  it('should fail when the capture pin or its table is missing or malformed', async () => {
    await annotate((value) => {
      delete value.sources[0]!.snapshot
    })
    expect(run()).toBe(1)
    expect(out).toContain('the capture pin (snapshot) is missing')

    await annotate((value) => {
      value.sources[0]!.snapshot = { revision: 'main', digests: {} }
    })
    expect(run()).toBe(1)
    expect(out).toContain('snapshot.revision) is not a full 40-hex commit')
    expect(out).toContain('no well-formed snapshot digest table')

    await annotate((value) => {
      value.sources[0]!.revision = 'HEAD'
    })
    expect(run()).toBe(1)
    expect(out).toContain('the code base (revision) is not a full 40-hex commit')
  })

  it('should ignore files outside library/ui/figma', async () => {
    await put('library/ui/README.md', 'edited\n')
    git('add', 'library/ui/README.md')

    expect(run()).toBe(0)
  })
})

describe('when the lock table itself is unusable', () => {
  it('should fail when the lock pins no files instead of passing vacuously', () => {
    expect(run()).toBe(1)
    expect(out).toContain('pins no ported files')
  })

  it('should fail on an empty table', async () => {
    const value = await lock()
    value.sources[0].files = {}
    await put(LOCK, JSON.stringify(value))

    expect(run()).toBe(1)
    expect(out).toContain('pins no ported files')
  })

  it('should fail on a malformed digest or a path outside the scope', () => {
    const source = (files: Record<string, unknown>) => ({
      sources: [{ repository: 'https://github.com/decentraland/figma-design-system', files }]
    })
    expect(() =>
      pinnedFiles(source({ 'library/ui/figma/a.ts': { sha256: 'x', fds: null } }))
    ).toThrow('malformed')
    expect(() =>
      pinnedFiles(source({ 'library/ui/other.ts': { sha256: 'a'.repeat(64), fds: null } }))
    ).toThrow('malformed')
    expect(() =>
      pinnedFiles(source({ 'library/ui/figma/a.ts': { sha256: 'a'.repeat(64), fds: { path: 1 } } }))
    ).toThrow('malformed')
    expect(() =>
      pinnedFiles(source({ 'library/ui/figma/../x': { sha256: 'a'.repeat(64), fds: null } }))
    ).toThrow('malformed')
  })

  it('should fail when the lock has no figma-design-system source', async () => {
    await put(LOCK, JSON.stringify({ sources: [] }))

    expect(run()).toBe(1)
    expect(out).toContain('no figma-design-system source')
  })

  it('should reject unknown arguments', () => {
    expect(run('--fix')).toBe(2)
  })
})

describe('when comparing tables', () => {
  it('should report every kind of difference, sorted', () => {
    expect(compare({ a: '1', b: '2', c: '3' }, { a: '1', b: 'x', d: '4' })).toEqual({
      mismatched: ['b'],
      missing: ['c'],
      unpinned: ['d']
    })
  })
})

describe('when the lock names the figma-design-system repository', () => {
  const FDS = 'https://github.com/decentraland/figma-design-system'

  it('should accept the decentraland repository', () => {
    expect(fdsSource({ sources: [{ repository: FDS }] })).toEqual({ repository: FDS })
  })

  it.each([
    'https://github.com/someone-else/figma-design-system',
    `${FDS}/../../someone-else/figma-design-system`,
    'http://github.com/decentraland/figma-design-system',
    'https://example.com/decentraland/figma-design-system'
  ])('should reject %s', (repository) => {
    expect(() => fdsSource({ sources: [{ repository }] })).toThrow(
      `must name the figma-design-system repository ${FDS}`
    )
  })

  it('should not take a suffixed URL for the figma-design-system source', () => {
    expect(() => fdsSource({ sources: [{ repository: `${FDS}.git` }] })).toThrow(
      'no figma-design-system source'
    )
  })

  it('should fail the check when the lock points at another owner', async () => {
    await put(
      LOCK,
      JSON.stringify({
        sources: [{ repository: 'https://github.com/someone-else/figma-design-system' }]
      })
    )

    expect(run()).toBe(1)
    expect(out).toContain('must name the figma-design-system repository')
  })
})
