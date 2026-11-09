import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { scanBuffer } from '../../key-guard.mjs'
import {
  FDS_SNAPSHOT,
  MARKER,
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
    JSON.stringify({ schema: 1, orphaned: [{ file: 'upstream-only' }] }) + '\n'
}

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
    'ledger/orphaned-assets.json': JSON.stringify({ schema: 1, orphaned: [] }) + '\n'
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
        'pages.json'
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
        'pages.json'
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

  it('should read the capture at the snapshot pin and the counterparts at the code base', async () => {
    const work = path.join(scratch, 'fds')
    await writeTree(work, { 'dcl-mcp-figma/src/a.ts': 'export const a = 2\n' })
    git(work, 'commit', '--quiet', '--no-gpg-sign', '-am', 'newer code')
    const newer = git(work, 'rev-parse', 'HEAD')
    git(work, 'push', '--quiet', remote, 'HEAD:refs/heads/newer')
    // The capture moves to the newer commit; the code base stays at the first one.
    await writeLock({ revision }, newer)
    await fetchCache({ seal: true })

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

  it("should overlay this tree's ledger and refuse one that carries more than asset paths and hashes", async () => {
    const asset = {
      path: 'figma/review/src/generated/components/assets/0a1b2c.svg',
      sha256: 'd'.repeat(64)
    }
    await writeTree(figmaRoot, {
      'ledger/orphaned-assets.json': JSON.stringify({ schema: 1, orphaned: [asset] }) + '\n'
    })
    await fetchCache({ seal: true })
    expect(
      JSON.parse(
        await readFile(path.join(figmaRoot, 'figma-snapshot/orphaned-assets.json'), 'utf8')
      )
    ).toEqual({ schema: 1, orphaned: [asset] })

    await writeTree(figmaRoot, {
      'ledger/orphaned-assets.json': JSON.stringify({
        schema: 1,
        orphaned: [{ ...asset, nodeId: '1:2' }]
      })
    })
    await expect(fetchCache({ seal: true })).rejects.toThrow('may hold only')
    await writeTree(figmaRoot, {
      'ledger/orphaned-assets.json': JSON.stringify({
        schema: 1,
        orphaned: [{ path: 'figma/figma-snapshot/components/Demo.json', sha256: 'd'.repeat(64) }]
      })
    })
    await expect(fetchCache({ seal: true })).rejects.toThrow('may hold only')
  })

  it('should fail when this tree has no ledger', async () => {
    await rm(path.join(figmaRoot, 'ledger'), { recursive: true })
    await expect(fetchCache()).rejects.toThrow()
  })

  it('should fail when the lock records another counterpart than the pinned commit holds', async () => {
    const lock = JSON.parse(await readFile(lockFile, 'utf8'))
    lock.sources[0].files['library/ui/figma/src/a.ts'].fds.sha256 = '0'.repeat(64)
    await writeFile(lockFile, JSON.stringify(lock))

    await expect(fetchCache()).rejects.toThrow(
      '1 ported files record another figma-design-system counterpart'
    )
  })

  it('should fetch again into a cache the consumer accepts', async () => {
    await rm(path.join(figmaRoot, 'figma-snapshot'), { recursive: true })

    expect(await fetchCache()).toMatchObject({ revision, files: 5, ported: 3, identical: 1 })
    await expect(
      verifyFdsCache(path.join(figmaRoot, 'figma-snapshot'), lockFile)
    ).resolves.toBeUndefined()
  })

  it('should refuse a capture that differs from the pinned digests and keep the previous cache', async () => {
    const lock = JSON.parse(await readFile(lockFile, 'utf8'))
    lock.sources[0].snapshot.digests['pages.json'] = '0'.repeat(64)
    await writeFile(lockFile, JSON.stringify(lock))
    const before = await readFile(path.join(figmaRoot, 'figma-snapshot/pages.json'))

    await expect(fetchCache()).rejects.toThrow('1 differing from the pinned digest')
    expect(await readFile(path.join(figmaRoot, 'figma-snapshot/pages.json'))).toEqual(before)
    expect(existsSync(`${path.join(figmaRoot, 'figma-snapshot')}.staging-${process.pid}`)).toBe(
      false
    )
  })

  it('should refuse a capture with a file the lock does not pin', async () => {
    const lock = JSON.parse(await readFile(lockFile, 'utf8'))
    delete lock.sources[0].snapshot.digests['icons/1-2.svg']
    await writeFile(lockFile, JSON.stringify(lock))

    await expect(fetchCache()).rejects.toThrow('1 unexpected')
  })

  describe('and a consumer reads the cache', () => {
    const dir = () => path.join(figmaRoot, 'figma-snapshot')

    it('should fail on an edited cache file', async () => {
      await writeFile(path.join(dir(), 'pages.json'), '{}\n')
      await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow(/differing.*fetch-source/)
    })

    it('should fail on an extra cache file', async () => {
      await writeFile(path.join(dir(), 'extra.json'), '{}\n')
      await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow(/unexpected.*fetch-source/)
    })

    it('should fail on a removed cache file', async () => {
      await rm(path.join(dir(), 'icons/1-2.svg'))
      await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow(/missing.*fetch-source/)
    })

    it('should fail when the lock now pins another capture commit', async () => {
      const lock = JSON.parse(await readFile(lockFile, 'utf8'))
      lock.sources[0].snapshot.revision = 'b'.repeat(40)
      await writeFile(lockFile, JSON.stringify(lock))
      await expect(verifyFdsCache(dir(), lockFile)).rejects.toThrow('another pin')
    })
  })
})

describe('when the cache or the pin is unusable', () => {
  it('should tell the consumer to fetch when there is no cache', async () => {
    await writeLock({ snapshot: { digests: { 'pages.json': 'a'.repeat(64) } } })
    await expect(verifyFdsCache(path.join(figmaRoot, 'figma-snapshot'), lockFile)).rejects.toThrow(
      'pnpm figma fetch-source'
    )
  })

  it('should fail when the pinned commit is not on the remote', async () => {
    await writeLock({ snapshot: { digests: { 'pages.json': 'a'.repeat(64) } } }, 'c'.repeat(40))
    await expect(fetchCache()).rejects.toThrow(/git fetch failed/)
    expect(existsSync(path.join(figmaRoot, 'figma-snapshot'))).toBe(false)
  })

  it('should refuse a lock without a full commit or without digests', () => {
    const lock = (source: Record<string, unknown>) => ({
      sources: [{ repository: 'https://github.com/decentraland/figma-design-system', ...source }]
    })
    const capture = { revision: 'a'.repeat(40) }
    expect(() => readFdsPin(lock({ revision: 'main', snapshot: capture }))).toThrow(
      'full 40-hex commit'
    )
    expect(() => readFdsPin(lock({ revision: 'a'.repeat(40) }))).toThrow('snapshot.revision')
    expect(() => readFdsPin(lock({ revision: 'a'.repeat(40), snapshot: capture }))).toThrow(
      'digest table'
    )
    expect(() =>
      readFdsPin(
        lock({
          revision: 'a'.repeat(40),
          snapshot: { ...capture, digests: { '../x': 'a'.repeat(64) } }
        })
      )
    ).toThrow('digest table')
    expect(() => readFdsPin({ sources: [] })).toThrow('no figma-design-system source')
  })

  it('should never print git output, which can echo capture content', async () => {
    await writeLock({ snapshot: { digests: { 'pages.json': 'a'.repeat(64) } } }, 'c'.repeat(40))
    const error = await fetchCache().catch((caught: Error) => caught)
    expect(String(error)).toMatch(/^Error: git fetch failed \(\d+\)$/)
  })
})

describe('when rewriting one capture file', () => {
  it('should keep the indentation and trailing newline of the source', () => {
    const pretty = keylessSnapshotFile('pages.json', Buffer.from('{\n "a": 1\n}\n'))
    expect(pretty.bytes.toString()).toBe('{\n "a": 1\n}\n')
    const compact = keylessSnapshotFile('variables.json', Buffer.from('{"a":1}'))
    expect(compact.bytes.toString()).toBe('{"a":1}')
  })

  it('should leave file names outside components untouched', () => {
    expect(keylessSnapshotFile('icons/a:b.svg', Buffer.from('<svg/>')).file).toBe('icons/a:b.svg')
  })
})
