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

// Registers, in the calling describe, a fresh Git repository with a ported tree and an unpinned lock,
// and captures what the check prints.
function useRepo() {
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
}

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
  let code: number

  useRepo()

  beforeEach(async () => {
    expect(run('--write')).toBe(0)
    await annotate()
  })

  it('should pin exactly the tracked files under library/ui/figma', async () => {
    expect(Object.keys((await lock()).sources[0].files)).toEqual([
      'library/ui/figma/a.ts',
      'library/ui/figma/src/b.ts'
    ])
  })

  describe('and nothing changed', () => {
    beforeEach(() => {
      code = run()
    })

    it('should pass', () => {
      expect(code).toBe(0)
    })

    it('should report that every pinned file matches', () => {
      expect(out).toContain('2 files match')
    })
  })

  describe('and an edited file is staged', () => {
    beforeEach(async () => {
      await put('library/ui/figma/a.ts', 'export const a = 3\n')
      git('add', 'library/ui/figma/a.ts')
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the file as changed', () => {
      expect(out).toContain('CHANGED library/ui/figma/a.ts')
    })
  })

  describe('and a pinned file is no longer tracked', () => {
    beforeEach(() => {
      git('rm', '--quiet', '--cached', '-f', 'library/ui/figma/src/b.ts')
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the file as missing', () => {
      expect(out).toContain('MISSING library/ui/figma/src/b.ts')
    })
  })

  describe('and a tracked file is not pinned', () => {
    beforeEach(async () => {
      await put('library/ui/figma/extra.ts', 'export {}\n')
      git('add', 'library/ui/figma/extra.ts')
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the file as unpinned', () => {
      expect(out).toContain('UNPINNED library/ui/figma/extra.ts')
    })
  })

  describe('and a Git LFS pointer is tracked', () => {
    beforeEach(async () => {
      await put(
        'library/ui/figma/font.woff2',
        'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'a'.repeat(64) + '\nsize 10\n'
      )
      git('add', 'library/ui/figma/font.woff2')
      code = run()
    })

    it('should fail instead of pinning its text', () => {
      expect(code).toBe(1)
    })

    it('should report the LFS pointer', () => {
      expect(out).toContain('is a Git LFS pointer')
    })
  })

  describe('and an edited file is resealed', () => {
    let writeCode: number

    beforeEach(async () => {
      await put('library/ui/figma/a.ts', 'export const a = 9\n')
      git('add', 'library/ui/figma/a.ts')
      writeCode = run('--write')
    })

    it('should reseal the lock', () => {
      expect(writeCode).toBe(0)
    })

    it('should keep counterparts and deviations', async () => {
      const entry = (await lock()).sources[0].files['library/ui/figma/a.ts']
      expect(entry).toMatchObject({ fds: null, deviation: 'dcl-one-only' })
    })

    describe('and the resealed lock is staged', () => {
      beforeEach(() => {
        git('add', LOCK)
        code = run()
      })

      it('should pass', () => {
        expect(code).toBe(0)
      })
    })
  })

  describe('and a new file is pinned without a deviation', () => {
    let writeCode: number

    beforeEach(async () => {
      await put('library/ui/figma/new.ts', 'export {}\n')
      git('add', 'library/ui/figma/new.ts')
      writeCode = run('--write')
      git('add', LOCK)
      code = run()
    })

    it('should pin the new file', () => {
      expect(writeCode).toBe(0)
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the missing deviation', () => {
      expect(out).toContain(
        'DEVIATION library/ui/figma/new.ts differs from figma-design-system and names no deviation'
      )
    })
  })

  describe('and a file is identical to its FDS blob', () => {
    describe('and it names no deviation', () => {
      beforeEach(async () => {
        await annotate((value) => {
          const entry = value.sources[0].files['library/ui/figma/a.ts']
          entry.fds = { path: 'dcl-mcp-figma/a.ts', sha256: entry.sha256 }
          delete entry.deviation
        })
        code = run()
      })

      it('should pass', () => {
        expect(code).toBe(0)
      })
    })

    describe('and it still names a deviation code', () => {
      beforeEach(async () => {
        await annotate((value) => {
          const entry = value.sources[0].files['library/ui/figma/a.ts']
          entry.fds = { path: 'dcl-mcp-figma/a.ts', sha256: entry.sha256 }
        })
        code = run()
      })

      it('should fail', () => {
        expect(code).toBe(1)
      })

      it('should report the deviation it still names', () => {
        expect(out).toContain('matches figma-design-system but still names a deviation')
      })
    })
  })

  describe('and a file names an unknown deviation code', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].files['library/ui/figma/a.ts'].deviation = 'made-up'
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the unknown code', () => {
      expect(out).toContain('names an unknown deviation code')
    })
  })

  describe('and a deviation code has a blank reason', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes['dcl-one-only'] = ' '
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the code without a reason', () => {
      expect(out).toContain('deviation code dcl-one-only has no reason')
    })
  })

  describe('and a deviation code has a reason that is not a string', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes = { 'dcl-one-only': 0 as unknown as string, note: null }
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the code without a reason', () => {
      expect(out).toContain('deviation code dcl-one-only has no reason')
    })
  })

  describe('and the note code carries a reason', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes!.note = 'shared'
      })
      code = run()
    })

    it('should fail, since each file carries its own note', () => {
      expect(code).toBe(1)
    })

    it('should report that the note code must map to null', () => {
      expect(out).toContain('deviation code note must map to null')
    })
  })

  describe('and the deviation codes are unsorted', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes = {
          note: null,
          'dcl-one-only': 'These files are owned by this tree.'
        }
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the unsorted codes', () => {
      expect(out).toContain('deviation codes are not in sorted order')
    })

    describe('and the lock is rewritten with --write', () => {
      let writeCode: number

      beforeEach(() => {
        writeCode = run('--write')
      })

      it('should rewrite the lock', () => {
        expect(writeCode).toBe(0)
      })

      it('should keep every reason unchanged', async () => {
        expect((await lock()).sources[0].deviationCodes).toEqual({
          'dcl-one-only': 'These files are owned by this tree.',
          note: null
        })
      })

      it('should sort the codes', async () => {
        expect(Object.keys((await lock()).sources[0].deviationCodes)).toEqual([
          'dcl-one-only',
          'note'
        ])
      })

      describe('and the sorted lock is staged', () => {
        beforeEach(() => {
          git('add', LOCK)
          out = ''
          code = run()
        })

        it('should pass', () => {
          expect(code).toBe(0)
        })
      })
    })
  })

  describe('and a reason is over 4000 characters', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes!['dcl-one-only'] = 'x'.repeat(4001)
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the reason as too long', () => {
      expect(out).toContain('deviation code dcl-one-only has a reason longer than 4000 characters')
    })

    it('should not report the reason as missing', () => {
      expect(out).not.toContain('has no reason')
    })
  })

  describe('and a code id is integer-like, which a JS object can never keep in string order', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes = {
          '10': 'Ten.',
          '9': 'Nine.',
          'dcl-one-only': 'These files are owned by this tree.',
          note: null
        }
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the single-digit code', () => {
      expect(out).toContain('deviation code 9 is integer-like')
    })

    it('should report the two-digit code', () => {
      expect(out).toContain('deviation code 10 is integer-like')
    })
  })

  describe('and a tracked path has a tab, so a sibling could hide an edited file', () => {
    beforeEach(async () => {
      await annotate()
      await put('library/ui/figma/a.ts\t.orig', 'export const a = 1\n')
      await put('library/ui/figma/a.ts', 'export const a = 9\n')
      git('add', '-A')
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the control character', () => {
      expect(out).toContain('control character in its path')
    })
  })

  describe('and the deviation code table is not an object', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0].deviationCodes = ['dcl-one-only'] as unknown as Record<string, string>
      })
      code = run()
    })

    it('should fail closed', () => {
      expect(code).toBe(1)
    })

    it('should report the malformed table', () => {
      expect(out).toContain('malformed deviationCodes table')
    })
  })

  describe.each(['constructor', 'toString', '__proto__'])(
    'and a file names inherited deviation %s',
    (inherited) => {
      let value: Lock

      beforeEach(async () => {
        await annotate((lock) => {
          lock.sources[0]!.files['library/ui/figma/a.ts']!.deviation = inherited
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

  describe('and a file names a deviation code declared as an own key with its reason', () => {
    let value: Lock

    beforeEach(async () => {
      await annotate((lock) => {
        const source = lock.sources[0]!
        source.deviationCodes = {
          'dcl-one-only': source.deviationCodes!['dcl-one-only']!,
          hardened: 'This file is hardened here.',
          note: null
        }
        source.files['library/ui/figma/a.ts']!.deviation = 'hardened'
      })
      value = await lock()
      code = run()
    })

    it('should accept the code through the real validator', () => {
      expect(pinnedFiles(value).problems).toEqual([])
    })

    it('should pass the check', () => {
      expect(code).toBe(0)
    })

    it('should not report an unknown deviation code', () => {
      expect(out).not.toContain('unknown deviation code')
    })
  })

  describe('and a file names the note code', () => {
    describe('and it carries no note', () => {
      beforeEach(async () => {
        await annotate((value) => {
          value.sources[0].files['library/ui/figma/a.ts'].deviation = 'note'
        })
        code = run()
      })

      it('should fail', () => {
        expect(code).toBe(1)
      })

      it('should require a short plain note', () => {
        expect(out).toContain('needs a short plain note')
      })
    })

    describe('and it carries a short plain note', () => {
      beforeEach(async () => {
        await annotate((value) => {
          Object.assign(value.sources[0].files['library/ui/figma/a.ts'], {
            deviation: 'note',
            note: 'Kept from the previous release for one consumer.'
          })
        })
        code = run()
      })

      it('should pass', () => {
        expect(code).toBe(0)
      })
    })
  })

  describe('and the capture pin is missing', () => {
    beforeEach(async () => {
      await annotate((value) => {
        delete value.sources[0]!.snapshot
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the missing capture pin', () => {
      expect(out).toContain('the capture pin (snapshot) is missing')
    })
  })

  describe('and the capture pin is malformed', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0]!.snapshot = { revision: 'main', digests: {} }
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report a capture revision that is not a full commit', () => {
      expect(out).toContain('snapshot.revision) is not a full 40-hex commit')
    })

    it('should report the malformed digest table', () => {
      expect(out).toContain('no well-formed snapshot digest table')
    })
  })

  describe('and the code base revision is not a full commit', () => {
    beforeEach(async () => {
      await annotate((value) => {
        value.sources[0]!.revision = 'HEAD'
      })
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the code base revision', () => {
      expect(out).toContain('the code base (revision) is not a full 40-hex commit')
    })
  })

  describe('and a file outside library/ui/figma is edited', () => {
    beforeEach(async () => {
      await put('library/ui/README.md', 'edited\n')
      git('add', 'library/ui/README.md')
      code = run()
    })

    it('should ignore the file and pass', () => {
      expect(code).toBe(0)
    })
  })
})

describe('when the lock table itself is unusable', () => {
  let code: number

  useRepo()

  describe('and the lock pins no files', () => {
    beforeEach(() => {
      code = run()
    })

    it('should fail instead of passing vacuously', () => {
      expect(code).toBe(1)
    })

    it('should report that the lock pins no ported files', () => {
      expect(out).toContain('pins no ported files')
    })
  })

  describe('and the files table is empty', () => {
    beforeEach(async () => {
      const value = await lock()
      value.sources[0].files = {}
      await put(LOCK, JSON.stringify(value))
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report that the lock pins no ported files', () => {
      expect(out).toContain('pins no ported files')
    })
  })

  describe('and a pinned entry is malformed or outside the scope', () => {
    const source = (files: Record<string, unknown>) => ({
      sources: [{ repository: 'https://github.com/decentraland/figma-design-system', files }]
    })

    it.each([
      ['a malformed digest', { 'library/ui/figma/a.ts': { sha256: 'x', fds: null } }],
      [
        'a path outside the scope',
        { 'library/ui/other.ts': { sha256: 'a'.repeat(64), fds: null } }
      ],
      [
        'a malformed FDS counterpart',
        { 'library/ui/figma/a.ts': { sha256: 'a'.repeat(64), fds: { path: 1 } } }
      ],
      [
        'a path escaping the scope',
        { 'library/ui/figma/../x': { sha256: 'a'.repeat(64), fds: null } }
      ]
    ])('should reject %s as malformed', (_, files) => {
      expect(() => pinnedFiles(source(files))).toThrow('malformed')
    })
  })

  describe('and the lock has no figma-design-system source', () => {
    beforeEach(async () => {
      await put(LOCK, JSON.stringify({ sources: [] }))
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the missing source', () => {
      expect(out).toContain('no figma-design-system source')
    })
  })

  describe('and the check gets an unknown argument', () => {
    it('should reject it with the usage exit code', () => {
      expect(run('--fix')).toBe(2)
    })
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

  describe('and the URL carries a suffix', () => {
    it('should not take it for the figma-design-system source', () => {
      expect(() => fdsSource({ sources: [{ repository: `${FDS}.git` }] })).toThrow(
        'no figma-design-system source'
      )
    })
  })

  describe('and the check runs on a lock that points at another owner', () => {
    let code: number

    useRepo()

    beforeEach(async () => {
      await put(
        LOCK,
        JSON.stringify({
          sources: [{ repository: 'https://github.com/someone-else/figma-design-system' }]
        })
      )
      code = run()
    })

    it('should fail', () => {
      expect(code).toBe(1)
    })

    it('should report the required repository', () => {
      expect(out).toContain('must name the figma-design-system repository')
    })
  })
})
