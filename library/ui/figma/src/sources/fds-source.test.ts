import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { scanBuffer } from '../../key-guard.mjs'
import {
  FDS_SNAPSHOT,
  FdsCacheError,
  MARKER,
  type Git,
  fetchFdsSource,
  keylessSnapshotFile,
  readFdsPin,
  verifyFdsCache
} from './fds-source.js'

// A local stand-in for figma-design-system: synthetic capture files, one carrying a fake file key and a Figma
// URL that the keyless rewrite must drop, and a page name Windows cannot store.
const FAKE_KEY = 'FakeKey0123456789abcdEF'
const sha = (text: string) => createHash('sha256').update(text).digest('hex')
const files: Record<string, string> = {
  'pages.json':
    JSON.stringify({ fileKey: FAKE_KEY, pages: [{ id: '1:1', name: 'Demo: Page' }] }, null, 2) +
    '\n',
  'components/Demo: Page.json':
    JSON.stringify({
      fileKey: FAKE_KEY,
      page: { id: '1:1', name: 'Demo: Page' },
      link: `https://www.figma.com/design/${FAKE_KEY}/Demo`,
      owned: [`${FDS_SNAPSHOT}/components/Demo.json#component:1:2`],
      components: []
    }) + '\n',
  'icons/1-2.svg': `<svg xmlns="http://www.w3.org/2000/svg"><!-- https://www.figma.com/file/${FAKE_KEY}/x --></svg>\n`,
  'icons-rows.txt': `1:2 https://www.figma.com/design/${FAKE_KEY}/Demo?node-id=1-2\n`,
  'orphaned-assets.json':
    JSON.stringify({ schema: 1, orphaned: [{ file: 'upstream-only' }] }) + '\n',
  'retirements.json':
    JSON.stringify({
      schema: 1,
      retired: [
        {
          pageId: '9:9',
          name: 'UpstreamOnly',
          reason: 'retired in figma-design-system',
          owned: [],
          sharedExclusions: []
        }
      ]
    }) + '\n'
}

const EMPTY_RETIREMENTS = JSON.stringify({ schema: 1, retired: [] }) + '\n'
const retirement = (overrides: Record<string, unknown> = {}) => ({
  pageId: '1:1',
  componentId: '1:2',
  name: 'Demo',
  reason: 'replaced by the rebuilt set',
  owned: ['figma/figma-snapshot/components/Demo%3A Page.json#component:1:2'],
  sharedExclusions: [],
  ...overrides
})

let scratch: string
let remote: string
let figmaRoot: string
let lockFile: string
let revision: string

const git = (cwd: string, ...args: string[]) =>
  execFileSync('git', ['-C', cwd, '-c', 'user.name=t', '-c', 'user.email=t@example.com', ...args], {
    stdio: 'pipe'
  })
    .toString()
    .trim()

async function writeTree(root: string, tree: Record<string, string>) {
  for (const [file, content] of Object.entries(tree)) {
    await mkdir(path.dirname(path.join(root, file)), { recursive: true })
    await writeFile(path.join(root, file), content)
  }
}

// Three ported files: one identical to its FDS blob, one adapted, one this tree owns.
const ported = () => ({
  'library/ui/figma/src/a.ts': { sha256: sha('export const a = 1\n'), fds: null },
  'library/ui/figma/review/src/b.ts': { sha256: sha('export const b = 3\n'), fds: null },
  'library/ui/figma/only-here.mjs': { sha256: sha('export {}\n'), fds: null }
})

// `pinned` is the capture commit; the code base defaults to it unless `extra.revision` names another.
async function writeLock(
  extra: { snapshot?: Record<string, unknown>; revision?: string } = {},
  pinned = revision
) {
  await writeFile(
    lockFile,
    JSON.stringify({
      sources: [
        {
          repository: 'https://github.com/decentraland/figma-design-system',
          revision: extra.revision ?? pinned,
          files: ported(),
          snapshot: { revision: pinned, ...extra.snapshot }
        }
      ]
    })
  )
}

const fetchCache = (options: { seal?: boolean } = {}) =>
  fetchFdsSource({ figmaRoot, lockFile, remote: `file://${remote}`, ...options })

beforeEach(async () => {
  scratch = await mkdtemp(path.join(tmpdir(), 'fds-source-test-'))
  const work = path.join(scratch, 'fds')
  await mkdir(work)
  git(work, 'init', '--quiet')
  await writeTree(
    work,
    Object.fromEntries(
      Object.entries(files).map(([file, content]) => [`${FDS_SNAPSHOT}/${file}`, content])
    )
  )
  await writeTree(work, {
    'README.md': 'not part of the capture\n',
    'dcl-mcp-figma/src/a.ts': 'export const a = 1\n',
    'dcl-ui-review/src/b.ts': 'export const b = 2\n'
  })
  git(work, 'add', '-A')
  git(work, 'commit', '--quiet', '--no-gpg-sign', '-m', 'capture')
  revision = git(work, 'rev-parse', 'HEAD')
  remote = path.join(scratch, 'fds.git')
  git(scratch, 'clone', '--quiet', '--bare', work, remote)
  git(remote, 'config', 'uploadpack.allowFilter', 'true')
  git(remote, 'config', 'uploadpack.allowAnySHA1InWant', 'true')

  figmaRoot = path.join(scratch, 'one/library/ui/figma')
  lockFile = path.join(scratch, 'one/library/ui/upstream.lock.json')
  await writeTree(figmaRoot, {
    'ledger/orphaned-assets.json': JSON.stringify({ schema: 1, orphaned: [] }) + '\n',
    'ledger/retirements.json': EMPTY_RETIREMENTS
  })
  await writeLock()
})

afterEach(async () => {
  await rm(scratch, { recursive: true, force: true })
})

async function listCache(): Promise<string[]> {
  const out: string[] = []
  const visit = async (prefix: string) => {
    for (const entry of await readdir(path.join(figmaRoot, 'figma-snapshot', prefix), {
      withFileTypes: true
    })) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name
      if (entry.isDirectory()) await visit(relative)
      else out.push(relative)
    }
  }
  await visit('')
  return out.sort()
}

describe('when sealing and then fetching the pinned capture', () => {
  beforeEach(async () => {
    await fetchCache({ seal: true })
  })

  it('should write the keyless capture and a digest table into the lock', async () => {
    expect(await listCache()).toEqual(
      [
        MARKER,
        'components/Demo%3A Page.json',
        'icons-rows.txt',
        'icons/1-2.svg',
        'orphaned-assets.json',
        'pages.json',
        'retirements.json'
      ].sort()
    )
    const page = await readFile(
      path.join(figmaRoot, 'figma-snapshot/components/Demo%3A Page.json'),
      'utf8'
    )
    expect(page).not.toContain(FAKE_KEY)
    expect(page).not.toContain('fileKey')
    expect(page).toContain('<DCL_FIGMA_FILE>')
    expect(page).toContain('figma/figma-snapshot/components/Demo.json')
    expect(
      JSON.parse(
        await readFile(path.join(figmaRoot, 'figma-snapshot/orphaned-assets.json'), 'utf8')
      )
    ).toEqual({ schema: 1, orphaned: [] })
    const pin = readFdsPin(JSON.parse(await readFile(lockFile, 'utf8')))
    expect(Object.keys(pin.digests).sort()).toEqual(
      [
        'components/Demo%3A Page.json',
        'icons-rows.txt',
        'icons/1-2.svg',
        'orphaned-assets.json',
        'pages.json',
        'retirements.json'
      ].sort()
    )
  })

  it('should strip a file key planted anywhere in the fetched capture before it reaches the cache', async () => {
    for (const file of await listCache()) {
      const bytes = await readFile(path.join(figmaRoot, 'figma-snapshot', file))
      expect(bytes.toString('utf8'), file).not.toContain(FAKE_KEY)
      expect(scanBuffer(bytes, FAKE_KEY), file).toEqual([])
    }
    const pages = JSON.parse(
      await readFile(path.join(figmaRoot, 'figma-snapshot/pages.json'), 'utf8')
    )
    expect(pages).not.toHaveProperty('fileKey')
  })

  it("should record each ported file's counterpart at the pinned commit, or none", async () => {
    const files = JSON.parse(await readFile(lockFile, 'utf8')).sources[0].files
    expect(files['library/ui/figma/src/a.ts'].fds).toEqual({
      path: 'dcl-mcp-figma/src/a.ts',
      sha256: sha('export const a = 1\n')
    })
    expect(files['library/ui/figma/review/src/b.ts'].fds).toEqual({
      path: 'dcl-ui-review/src/b.ts',
      sha256: sha('export const b = 2\n')
    })
    expect(files['library/ui/figma/only-here.mjs'].fds).toBeNull()
  })

  describe('and the capture moves to a newer commit while the code base stays at the first one', () => {
    let newer: string

    beforeEach(async () => {
      const work = path.join(scratch, 'fds')
      await writeTree(work, { 'dcl-mcp-figma/src/a.ts': 'export const a = 2\n' })
      git(work, 'commit', '--quiet', '--no-gpg-sign', '-am', 'newer code')
      newer = git(work, 'rev-parse', 'HEAD')
      git(work, 'push', '--quiet', remote, 'HEAD:refs/heads/newer')
      await writeLock({ revision }, newer)
      await fetchCache({ seal: true })
    })

    it('should read the capture at the snapshot pin and the counterparts at the code base', async () => {
      const lock = JSON.parse(await readFile(lockFile, 'utf8'))
      expect(lock.sources[0].revision).toBe(revision)
      expect(lock.sources[0].snapshot.revision).toBe(newer)
      expect(lock.sources[0].files['library/ui/figma/src/a.ts'].fds.sha256).toBe(
        sha('export const a = 1\n')
      )
      const marker = JSON.parse(
        await readFile(path.join(figmaRoot, 'figma-snapshot', MARKER), 'utf8')
      )
      expect(marker.revision).toBe(newer)
    })
  })

  describe("and this tree's ledger lists an orphaned asset", () => {
    let asset: { path: string; sha256: string }

    beforeEach(() => {
      asset = {
        path: 'figma/review/src/generated/components/assets/0a1b2c.svg',
        sha256: 'd'.repeat(64)
      }
    })

    describe('and it carries only the asset path and hash', () => {
      beforeEach(async () => {
        await writeTree(figmaRoot, {
          'ledger/orphaned-assets.json': JSON.stringify({ schema: 1, orphaned: [asset] }) + '\n'
        })
        await fetchCache({ seal: true })
      })

      it("should overlay this tree's ledger onto the cache", async () => {
        expect(
          JSON.parse(
            await readFile(path.join(figmaRoot, 'figma-snapshot/orphaned-assets.json'), 'utf8')
          )
        ).toEqual({ schema: 1, orphaned: [asset] })
      })
    })

    describe('and it carries more than the asset path and hash', () => {
      beforeEach(async () => {
        await writeTree(figmaRoot, {
          'ledger/orphaned-assets.json': JSON.stringify({
            schema: 1,
            orphaned: [{ ...asset, nodeId: '1:2' }]
          })
        })
      })

      it('should refuse the ledger', async () => {
        await expect(fetchCache({ seal: true })).rejects.toThrow('may hold only')
      })
    })

    describe('and its path is not an asset path', () => {
      beforeEach(async () => {
        await writeTree(figmaRoot, {
          'ledger/orphaned-assets.json': JSON.stringify({
            schema: 1,
            orphaned: [
              { path: 'figma/figma-snapshot/components/Demo.json', sha256: 'd'.repeat(64) }
            ]
          })
        })
      })

      it('should refuse the ledger', async () => {
        await expect(fetchCache({ seal: true })).rejects.toThrow('may hold only')
      })
    })
  })

  it("should cache this tree's empty retirement ledger instead of figma-design-system's", async () => {
    expect(
      JSON.parse(await readFile(path.join(figmaRoot, 'figma-snapshot/retirements.json'), 'utf8'))
    ).toEqual({ schema: 1, retired: [] })
  })

  describe("and this tree's retirement ledger holds a record", () => {
    let ledger: string

    beforeEach(async () => {
      ledger = JSON.stringify({ schema: 1, retired: [retirement()] }, null, 2) + '\n'
      await writeTree(figmaRoot, { 'ledger/retirements.json': ledger })
      await fetchCache({ seal: true })
    })

    it("should overlay it over figma-design-system's, so one ledger is effective", async () => {
      expect(await readFile(path.join(figmaRoot, 'figma-snapshot/retirements.json'), 'utf8')).toBe(
        ledger
      )
    })

    it('should pin its digest in the lock', async () => {
      expect(
        readFdsPin(JSON.parse(await readFile(lockFile, 'utf8'))).digests['retirements.json']
      ).toBe(sha(ledger))
    })
  })

  it.each([
    ['a malformed entry', { schema: 1, retired: [{ pageId: '1:1' }] }],
    ['an unknown key', { schema: 1, retired: [{ ...retirement(), fileKey: 'x' }] }],
    ['an unknown top-level key', { schema: 1, retired: [], note: 'x' }],
    [
      'a Figma URL in a reason',
      {
        schema: 1,
        retired: [retirement({ reason: 'see https://www.figma' + `.com/design/${FAKE_KEY}/x` })]
      }
    ],
    ['a link in a name', { schema: 1, retired: [retirement({ name: 'figma.com/x' })] }],
    ['a node id that is a URL', { schema: 1, retired: [retirement({ pageId: 'https://x' })] }],
    [
      'a path that leaves the repository',
      { schema: 1, retired: [retirement({ owned: ['figma/../../etc/passwd'] })] }
    ]
  ])('should refuse a retirement ledger with %s and keep the previous cache', async (_, ledger) => {
    const before = await listCache()
    await writeTree(figmaRoot, { 'ledger/retirements.json': JSON.stringify(ledger) })
    await expect(fetchCache({ seal: true })).rejects.toThrow('may hold only')
    expect(await listCache()).toEqual(before)
    await expect(
      readFile(path.join(figmaRoot, 'figma-snapshot/retirements.json'), 'utf8')
    ).resolves.toBe(EMPTY_RETIREMENTS)
  })

  describe("and this tree's retirement ledger is not JSON", () => {
    beforeEach(async () => {
      await writeTree(figmaRoot, { 'ledger/retirements.json': '{ not json' })
    })

    it('should refuse the ledger', async () => {
      await expect(fetchCache({ seal: true })).rejects.toThrow(
        'ledger/retirements.json is not valid JSON'
      )
    })
  })

  describe('and this tree has no retirement ledger', () => {
    beforeEach(async () => {
      await rm(path.join(figmaRoot, 'ledger/retirements.json'))
    })

    it('should fail, naming the missing ledger', async () => {
      await expect(fetchCache()).rejects.toThrow(/ENOENT.*ledger\/retirements\.json/)
    })
  })

  describe('and this tree has no ledger folder', () => {
    beforeEach(async () => {
      await rm(path.join(figmaRoot, 'ledger'), { recursive: true })
    })

    it('should fail, naming the first missing ledger', async () => {
      await expect(fetchCache()).rejects.toThrow(/ENOENT.*ledger\/orphaned-assets\.json/)
    })
  })

  // Rewrites the sealed lock in place through `edit`.
  type SealedLock = {
    sources: {
      files: Record<string, { fds: { sha256: string } }>
      snapshot: { revision: string; digests: Record<string, string> }
    }[]
  }
  const editLock = async (edit: (lock: SealedLock) => void) => {
    const lock = JSON.parse(await readFile(lockFile, 'utf8')) as SealedLock
    edit(lock)
    await writeFile(lockFile, JSON.stringify(lock))
  }

  describe('and the lock records another counterpart than the pinned commit holds', () => {
    beforeEach(async () => {
      await editLock((lock) => {
        lock.sources[0]!.files['library/ui/figma/src/a.ts']!.fds.sha256 = '0'.repeat(64)
      })
    })

    it('should fail the fetch', async () => {
      await expect(fetchCache()).rejects.toThrow(
        '1 ported files record another figma-design-system counterpart'
      )
    })
  })

  describe('and the cache is removed', () => {
    beforeEach(async () => {
      await rm(path.join(figmaRoot, 'figma-snapshot'), { recursive: true })
    })

    it('should fetch again into a cache the consumer accepts', async () => {
      expect(await fetchCache()).toMatchObject({ revision, files: 6, ported: 3, identical: 1 })
      await expect(
        verifyFdsCache(path.join(figmaRoot, 'figma-snapshot'), lockFile)
      ).resolves.toBeUndefined()
    })
  })

  describe('and the capture differs from the pinned digests', () => {
    let before: Buffer

    beforeEach(async () => {
      await editLock((lock) => {
        lock.sources[0]!.snapshot.digests['pages.json'] = '0'.repeat(64)
      })
      before = await readFile(path.join(figmaRoot, 'figma-snapshot/pages.json'))
    })

    it('should refuse the capture and keep the previous cache', async () => {
      await expect(fetchCache()).rejects.toThrow('1 differing from the pinned digest')
      expect(await readFile(path.join(figmaRoot, 'figma-snapshot/pages.json'))).toEqual(before)
      expect(existsSync(`${path.join(figmaRoot, 'figma-snapshot')}.staging-${process.pid}`)).toBe(
        false
      )
    })
  })

  describe('and the capture holds a file the lock does not pin', () => {
    beforeEach(async () => {
      await editLock((lock) => {
        delete lock.sources[0]!.snapshot.digests['icons/1-2.svg']
      })
    })

    it('should refuse the capture', async () => {
      await expect(fetchCache()).rejects.toThrow('1 unexpected')
    })
  })

  describe('and a consumer reads the cache', () => {
    const dir = () => path.join(figmaRoot, 'figma-snapshot')

    describe('and a cache file was edited', () => {
      beforeEach(async () => {
        await writeFile(path.join(dir(), 'pages.json'), '{}\n')
      })

      it('should fail, pointing at fetch-source', async () => {
        await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow(/differing.*fetch-source/)
      })
    })

    describe('and the cache holds an extra file', () => {
      beforeEach(async () => {
        await writeFile(path.join(dir(), 'extra.json'), '{}\n')
      })

      it('should fail, pointing at fetch-source', async () => {
        await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow(/unexpected.*fetch-source/)
      })
    })

    describe('and a cache file was removed', () => {
      beforeEach(async () => {
        await rm(path.join(dir(), 'icons/1-2.svg'))
      })

      it('should fail, pointing at fetch-source', async () => {
        await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow(/missing.*fetch-source/)
      })
    })

    describe('and the lock now pins another capture commit', () => {
      beforeEach(async () => {
        await editLock((lock) => {
          lock.sources[0]!.snapshot.revision = 'b'.repeat(40)
        })
      })

      it('should fail on the changed pin', async () => {
        await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow('another pin')
      })
    })
  })
})

describe('when the cache or the pin is unusable', () => {
  describe('and there is no cache', () => {
    beforeEach(async () => {
      await writeLock({ snapshot: { digests: { 'pages.json': 'a'.repeat(64) } } })
    })

    it('should tell the consumer to fetch', async () => {
      await expect(
        verifyFdsCache(path.join(figmaRoot, 'figma-snapshot'), lockFile)
      ).rejects.toThrow('pnpm figma fetch-source')
    })
  })

  describe('and the pinned commit is not on the remote', () => {
    beforeEach(async () => {
      await writeLock({ snapshot: { digests: { 'pages.json': 'a'.repeat(64) } } }, 'c'.repeat(40))
    })

    it('should fail the fetch and write no cache', async () => {
      await expect(fetchCache()).rejects.toThrow(/git fetch failed/)
      expect(existsSync(path.join(figmaRoot, 'figma-snapshot'))).toBe(false)
    })

    it('should never print git output, which can echo capture content', async () => {
      const error = await fetchCache().catch((caught: Error) => caught)
      expect(String(error)).toMatch(/^Error: git fetch failed \(\d+\)$/)
    })
  })

  describe('and the lock lacks a full commit or a digest table', () => {
    const lock = (source: Record<string, unknown>) => ({
      sources: [{ repository: 'https://github.com/decentraland/figma-design-system', ...source }]
    })

    it.each([
      [
        'a branch name instead of a commit',
        lock({ revision: 'main', snapshot: { revision: 'a'.repeat(40) } }),
        'full 40-hex commit'
      ],
      ['no snapshot pin', lock({ revision: 'a'.repeat(40) }), 'snapshot.revision'],
      [
        'no digest table',
        lock({ revision: 'a'.repeat(40), snapshot: { revision: 'a'.repeat(40) } }),
        'digest table'
      ],
      [
        'a digest table with a path that leaves the capture',
        lock({
          revision: 'a'.repeat(40),
          snapshot: { revision: 'a'.repeat(40), digests: { '../x': 'a'.repeat(64) } }
        }),
        'digest table'
      ],
      ['no figma-design-system source', { sources: [] }, 'no figma-design-system source']
    ])('should refuse a lock with %s', (_, value, message) => {
      expect(() => readFdsPin(value)).toThrow(message)
    })
  })
})

describe('when the pinned tree lists a path outside the snapshot folder', () => {
  // A real git tree cannot hold these names, so the listing is rewritten on its way out of `git ls-tree`.
  const listing =
    (planted: string): Git =>
    async (args, options) => {
      const out = execFileSync('git', args, {
        input: options?.input,
        maxBuffer: 1 << 30,
        stdio: ['pipe', 'pipe', 'pipe']
      })
      if (!args.includes('ls-tree') || !args.includes(FDS_SNAPSHOT)) return out
      const first = out.toString('utf8').split('\0')[0]!
      const meta = first.split('\t')[0]!
      return Buffer.concat([out, Buffer.from(`${meta}\t${planted}\0`)])
    }

  it.each([
    `${FDS_SNAPSHOT}/../../escape.json`,
    `${FDS_SNAPSHOT}/components/../../../escape.json`,
    `${FDS_SNAPSHOT}/./pages.json`,
    `${FDS_SNAPSHOT}//pages.json`,
    `${FDS_SNAPSHOT}/a\\..\\..\\escape.json`,
    `${FDS_SNAPSHOT}/`,
    'elsewhere/escape.json'
  ])('should refuse %s with FdsCacheError, sealed or not, and write nothing', async (planted) => {
    const attempt = (seal: boolean) =>
      fetchFdsSource({
        figmaRoot,
        lockFile,
        remote: `file://${remote}`,
        runner: listing(planted),
        seal
      }).catch((caught: Error) => caught)
    const refused = (error: unknown) => {
      expect(error).toBeInstanceOf(FdsCacheError)
      expect(String(error)).toContain('outside the snapshot folder')
      expect(existsSync(path.join(scratch, 'one/library/escape.json'))).toBe(false)
      expect(existsSync(path.join(scratch, 'one/escape.json'))).toBe(false)
    }
    // --seal skips the digest table, so it must refuse on its own.
    refused(await attempt(true))
    expect(existsSync(path.join(figmaRoot, 'figma-snapshot'))).toBe(false)
    // Against a pinned table and an existing cache, the cache is kept as it was.
    await fetchCache({ seal: true })
    const before = await listCache()
    refused(await attempt(false))
    expect(await listCache()).toEqual(before)
  })
})

describe('when rewriting one capture file', () => {
  it.each([
    ['indented with a trailing newline', 'pages.json', '{\n "a": 1\n}\n'],
    ['compact without a trailing newline', 'variables.json', '{"a":1}']
  ])('should keep the layout of a source %s', (_, file, source) => {
    expect(keylessSnapshotFile(file, Buffer.from(source)).bytes.toString()).toBe(source)
  })

  it('should leave file names outside components untouched', () => {
    expect(keylessSnapshotFile('icons/a:b.svg', Buffer.from('<svg/>')).file).toBe('icons/a:b.svg')
  })
})

describe('when the cache swap fails after the old cache was moved aside', () => {
  let before: Map<string, Buffer>

  const readCache = async () =>
    new Map(
      await Promise.all(
        (await listCache()).map(
          async (file) =>
            [file, await readFile(path.join(figmaRoot, 'figma-snapshot', file))] as const
        )
      )
    )

  beforeEach(async () => {
    await fetchCache({ seal: true })
    before = await readCache()
  })

  describe('and the old cache can be moved back', () => {
    let failingRename: (from: string, to: string) => Promise<void>

    beforeEach(() => {
      failingRename = async (from, to) => {
        if (from.includes('.staging-')) throw new Error('injected rename failure')
        await rename(from, to)
      }
    })

    it('should restore the previous cache byte for byte and leave no previous folder behind', async () => {
      await expect(
        fetchFdsSource({ figmaRoot, lockFile, remote: `file://${remote}`, rename: failingRename })
      ).rejects.toThrow('injected rename failure')
      expect(await readCache()).toEqual(before)
      expect((await readdir(figmaRoot)).filter((entry) => entry.includes('.previous-'))).toEqual([])
      expect((await readdir(figmaRoot)).filter((entry) => entry.includes('.staging-'))).toEqual([])
    })
  })

  describe('and moving the old cache back also fails', () => {
    let restoreError: Error
    let failingRename: (from: string, to: string) => Promise<void>

    beforeEach(() => {
      restoreError = new Error('injected restore failure')
      failingRename = async (from, to) => {
        if (from.includes('.staging-')) throw new Error('injected rename failure')
        if (from.includes('.previous-')) throw restoreError
        await rename(from, to)
      }
    })

    it('should keep the original failure as the cause and name the backup', async () => {
      const error = await fetchFdsSource({
        figmaRoot,
        lockFile,
        remote: `file://${remote}`,
        rename: failingRename
      }).catch((caught: unknown) => caught)
      expect(error).toBeInstanceOf(FdsCacheError)
      expect(((error as Error).cause as Error).message).toBe('injected rename failure')
      expect((error as { restoreError?: unknown }).restoreError).toBe(restoreError)
      const backups = (await readdir(figmaRoot)).filter((entry) => entry.includes('.previous-'))
      expect(backups).toHaveLength(1)
      const backup = path.join(figmaRoot, backups[0]!, 'figma-snapshot')
      expect((error as Error).message).toContain(backup)
      expect(existsSync(path.join(figmaRoot, 'figma-snapshot'))).toBe(false)
      for (const [file, bytes] of before)
        expect(await readFile(path.join(backup, file))).toEqual(bytes)
    })
  })
})

describe('when a killed run left a backup folder with this process id behind', () => {
  const stale = () => path.join(figmaRoot, `figma-snapshot.previous-${process.pid}`)

  beforeEach(async () => {
    await fetchCache({ seal: true })
    await mkdir(path.join(stale(), 'figma-snapshot'), { recursive: true })
    await writeFile(path.join(stale(), 'figma-snapshot', 'left-behind.json'), '{"kept":true}\n')
  })

  it('should still swap the cache in and leave the stale folder untouched', async () => {
    await fetchCache()
    expect(existsSync(path.join(figmaRoot, 'figma-snapshot', '.fds-source.json'))).toBe(true)
    expect(await readFile(path.join(stale(), 'figma-snapshot', 'left-behind.json'), 'utf8')).toBe(
      '{"kept":true}\n'
    )
    expect((await readdir(figmaRoot)).filter((entry) => entry.includes('.previous-'))).toEqual([
      path.basename(stale())
    ])
  })
})

describe('when the lock names the figma-design-system repository', () => {
  const FDS = 'https://github.com/decentraland/figma-design-system'
  const lock = (repository: string) => ({
    sources: [{ repository, revision: 'a'.repeat(40), snapshot: { revision: 'b'.repeat(40) } }]
  })

  it('should fetch the decentraland repository over SSH', () => {
    expect(readFdsPin(lock(FDS), { sealing: true }).remote).toBe(
      'git@github.com:decentraland/figma-design-system.git'
    )
  })

  it.each([
    'https://github.com/someone-else/figma-design-system',
    `${FDS}/../../someone-else/figma-design-system`,
    'http://github.com/decentraland/figma-design-system',
    'https://example.com/decentraland/figma-design-system'
  ])('should refuse %s', (repository) => {
    expect(() => readFdsPin(lock(repository), { sealing: true })).toThrow(
      `must name the figma-design-system repository ${FDS}`
    )
  })

  describe('and the URL carries a .git suffix', () => {
    it('should not take it for the figma-design-system source', () => {
      expect(() => readFdsPin(lock(`${FDS}.git`), { sealing: true })).toThrow(
        'no figma-design-system source'
      )
    })
  })
})
