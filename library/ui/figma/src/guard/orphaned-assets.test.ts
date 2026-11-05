import { execFileSync, spawnSync } from 'node:child_process'
import { cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createApp, runDiff, runSync, type App } from '../app.js'
import { writePromotionManifest } from '../promotion/manifest.js'
import { LEDGER } from './consumers.js'
import { lifecycleFixture, fixtureBytes, put, json, SNAP, GEN } from './lifecycle-fixture.js'
import { checkLedger, loadLedger, serializeLedger, sha256 } from './orphaned-assets.js'
import { checkScope } from './check-scope.js'
import { emptyRetirements } from './retirements.js'
import { GUARDED_DIRECTORIES, buildOwnership, ownerOf } from './ownership.js'
import { listFiles, toPosix } from './fs-safe.js'

const ASSETS = 'figma/review/src/generated/components/assets'
const temps: string[] = []
const tempDir = async (prefix: string) => {
  const dir = await mkdtemp(path.join(os.tmpdir(), prefix))
  temps.push(dir)
  return dir
}
const copyTree = async (from: string, prefix: string) => {
  const to = await tempDir(prefix)
  await cp(from, to, { recursive: true })
  return to
}

// A recapture of page 1:1 that changes every component's artwork, so the asset they shared stops being
// generated: the shape of the PlaceCard case (library copy, CSS ../assets URLs, integrity manifests).
async function recapture(root: string, stroke: string) {
  const page = await json(root, `${SNAP}/components/Page.json`)
  for (const component of page.components)
    component.base.tree.svg = component.base.tree.svg.replaceAll(
      /<path d="[^"]*"\/>/g,
      `<path d="${stroke}"/>`
    )
  await put(root, `${SNAP}/components/Page.json`, page)
  const app = await createApp(path.join(root, 'figma'))
  // As live capture runs it; its guard then validates the ledger against the base before publishing.
  const report = await runSync(app, {
    source: 'snapshot',
    capturedPageIds: ['1:1'],
    ledgerOrphans: true
  })
  return { app, report }
}

async function scenario() {
  const { root: base } = await lifecycleFixture()
  temps.push(base)
  const head = await copyTree(base, 'orphan-head-')
  const { app, report } = await recapture(head, 'M1 1h18v18H1z')
  const orphan = report.orphaned?.[0]
  if (!orphan) throw new Error('The recapture did not orphan the shared asset')
  return { base, head, app, orphan }
}

const ledgerCheck = (baseDir: string, headDir: string, developer = false, partial = false) =>
  checkLedger({ baseDir, headDir, developer, partial })
const causes = async (...args: Parameters<typeof ledgerCheck>) =>
  (await ledgerCheck(...args)).violations.map((violation) => violation.cause)

afterEach(async () => {
  for (const dir of temps.splice(0)) await rm(dir, { recursive: true, force: true })
})

describe('when a sync other than live capture stops generating an asset', () => {
  it('should not ledger it, leaving it removed so cli diff fails', async () => {
    const { root: base } = await lifecycleFixture()
    temps.push(base)
    const page = await json(base, `${SNAP}/components/Page.json`)
    for (const component of page.components)
      component.base.tree.svg = component.base.tree.svg.replaceAll(
        /<path d="[^"]*"\/>/g,
        '<path d="M3 3h14v14H3z"/>'
      )
    await put(base, `${SNAP}/components/Page.json`, page)
    const app = await createApp(path.join(base, 'figma'))
    const report = await runSync(app, { source: 'snapshot', capturedPageIds: ['1:1'] })
    expect(report.orphaned).toBeUndefined()
    await expect(readFile(path.join(base, LEDGER))).rejects.toMatchObject({ code: 'ENOENT' })
    expect((await runDiff(app, { source: 'snapshot' })).diff.removed.length).toBeGreaterThan(0)
  })
})

describe('when a recapture stops generating an asset', () => {
  let base: string
  let head: string
  let orphan: string
  let app: App
  beforeEach(async () => {
    ;({ base, head, orphan, app } = await scenario())
  })

  it('should ledger it with path and sha256 only and keep its bytes', async () => {
    const bytes = await readFile(path.join(base, orphan))
    expect(await loadLedger(head)).toEqual({
      schema: 1,
      orphaned: [{ path: orphan, sha256: sha256(bytes) }]
    })
    expect((await readFile(path.join(head, orphan))).equals(bytes)).toBe(true)
  })

  it('should admit the entry against the base in the guard', async () => {
    expect(await causes(base, head)).toEqual([])
  })

  it('should admit it after the review promotion manifest is regenerated', async () => {
    await writePromotionManifest(app)
    const manifest = await json(head, 'figma/review/src/review/promotion.json')
    expect(Object.keys(manifest.files)).toContain(orphan)
    expect(await causes(base, head)).toEqual([])
  })

  it('should report it as retained, not removed, in diff', async () => {
    const report = await runDiff(app, { source: 'snapshot' })
    expect(report.diff.removed).toEqual([])
    expect(report.retained).toEqual([orphan.slice(`${GEN.replace('/components', '')}/`.length)])
    expect(report.ledgerIssues).toEqual([])
  })

  it('should report diff drift when the ledgered asset is used again', async () => {
    await put(
      head,
      'figma/review/src/Hero.tsx',
      `import a from './generated/components/assets/${path.basename(orphan)}'`
    )
    expect((await runDiff(app, { source: 'snapshot' })).ledgerIssues).toEqual([
      `${orphan}: ledgered asset is used again, reactivate it`
    ])
  })

  it('should pass the complete automated guard with the ledgered orphan present', async () => {
    const result = await checkScope({
      baseDir: base,
      stagedDir: head,
      capturedPageIds: ['1:1'],
      runKind: 'automated',
      stage: 'complete',
      retirements: emptyRetirements(),
      directories: [SNAP, GEN],
      files: []
    })
    expect(
      result.violations.filter((item) => item.kind === 'orphan-ledger' || item.kind === 'removed')
    ).toEqual([])
  })

  describe('and something still uses it', () => {
    it.each([
      [
        'a foreign JSX import',
        'figma/review/src/Hero.jsx',
        (name: string) => `import a from './generated/components/assets/${name}'`
      ],
      [
        'a consumer in another root',
        'scripts/use.mjs',
        (name: string) => `read('../figma/review/src/generated/components/assets/${name}')`
      ],
      ['an unresolved basename in markdown', 'docs/note.md', (name: string) => `art ${name}`],
      [
        'an unresolved basename beside both copies',
        'components/x.ts',
        (name: string) => `const a = '${name}'`
      ],
      [
        'a library CSS with a resolved and an unresolved reference',
        'components/page/Mixed.module.css',
        (name: string) => `a { background: url(../assets/${name}) } b { background: url(${name}) }`
      ]
    ])('should reject admission for %s', async (_, file, content) => {
      await put(head, file, content(path.basename(orphan)))
      expect((await causes(base, head)).join('\n')).toContain('the asset is still used by')
    })

    it('should reject a path elsewhere in the review promotion manifest', async () => {
      const manifest = await json(head, 'figma/review/src/review/promotion.json')
      await put(head, 'figma/review/src/review/promotion.json', { ...manifest, note: orphan })
      expect((await causes(base, head)).join('\n')).toContain(
        'figma/review/src/review/promotion.json'
      )
    })

    it('should fail closed on an unreadable text file', async () => {
      await mkdir(path.join(head, 'docs'), { recursive: true })
      await writeFile(path.join(head, 'docs/bad.md'), Buffer.from([0xff, 0xfe, 0x00]))
      expect((await causes(base, head)).join('\n')).toContain('consumers could not be checked')
    })
  })

  describe('and the stage is partial without .git', () => {
    let staged: string
    beforeEach(async () => {
      staged = await tempDir('orphan-partial-')
      for (const directory of [SNAP, 'figma/review'])
        await cp(path.join(head, directory), path.join(staged, directory), { recursive: true })
    })

    it('should admit a zero-consumer orphan, inheriting unstaged roots from the base', async () => {
      expect(await causes(base, staged, false, true)).toEqual([])
    })

    it('should reject it when a newly generated file uses it', async () => {
      await put(
        staged,
        `${GEN}/page/New.module.css`,
        `a { background: url(../assets/${path.basename(orphan)}) }`
      )
      expect((await causes(base, staged, false, true)).join('\n')).toContain(
        `${GEN}/page/New.module.css`
      )
    })
  })

  describe('and a second recapture orphans another asset over a non-empty ledger', () => {
    it('should admit the new entry and keep the first one', async () => {
      const next = await copyTree(head, 'orphan-next-')
      const { report } = await recapture(next, 'M2 2h16v16H2z')
      expect(report.orphaned).toHaveLength(1)
      expect((await loadLedger(next)).orphaned.map((entry) => entry.path)).toEqual(
        [orphan, report.orphaned![0]!].sort()
      )
      expect(await causes(head, next)).toEqual([])
    })
  })
})

describe('when a ledger entry is not a real recapture orphan', () => {
  let base: string
  let head: string
  let orphan: string
  beforeEach(async () => {
    ;({ base, head, orphan } = await scenario())
  })
  const forge = async (dir: string, entries: { path: string; sha256: string }[]) =>
    put(dir, LEDGER, serializeLedger(entries))

  it('should reject an addition whose owners were not recaptured', async () => {
    const forged = await copyTree(base, 'orphan-forged-')
    const name = path.basename(orphan)
    for (const file of ['page/First.module.css', 'page/Second.module.css'])
      await put(
        forged,
        `${GEN}/${file}`,
        (await readFile(path.join(forged, GEN, file), 'utf8')).replaceAll(name, 'gone.svg')
      )
    await forge(forged, [{ path: orphan, sha256: sha256(await readFile(path.join(base, orphan))) }])
    expect((await causes(base, forged)).join('\n')).toContain('no recapture changed its owner')
  })

  it('should reject a forged addition for an asset that is still used', async () => {
    const forged = await copyTree(base, 'orphan-forged-')
    await forge(forged, [{ path: orphan, sha256: sha256(await readFile(path.join(base, orphan))) }])
    expect((await causes(base, forged)).join('\n')).toContain('the asset is still used by')
  })

  it('should reject an automated addition owned by a page the run did not capture', async () => {
    const result = await checkLedger({
      baseDir: base,
      headDir: head,
      partial: false,
      developer: false,
      captured: new Set(['9:9'])
    })
    expect(result.violations.map((violation) => violation.cause)).toEqual([
      'owned by pages this run did not capture: 1:1'
    ])
  })

  it('should reject an edit of an existing entry', async () => {
    const next = await copyTree(head, 'orphan-edit-')
    await forge(next, [{ path: orphan, sha256: 'a'.repeat(64) }])
    expect(await causes(head, next)).toEqual(['an existing ledger entry was edited'])
  })

  it('should reject removing an entry outside a developer transition', async () => {
    const next = await copyTree(head, 'orphan-remove-')
    await forge(next, [])
    await rm(path.join(next, orphan))
    expect(await causes(head, next)).toEqual([
      'only a developer reactivation or drain may remove a ledger entry'
    ])
  })

  it('should reject an unowned asset', async () => {
    const stray = `${ASSETS}/ffffffffffffffff.svg`
    await put(base, stray, '<svg/>')
    await put(head, stray, '<svg/>')
    await forge(head, [
      { path: orphan, sha256: sha256(await readFile(path.join(base, orphan))) },
      { path: stray, sha256: sha256(Buffer.from('<svg/>')) }
    ])
    expect(await causes(base, head)).toEqual(['the asset had no native generated owner at base'])
  })

  it('should reject a sha256 that does not match the base bytes', async () => {
    await forge(head, [{ path: orphan, sha256: 'b'.repeat(64) }])
    expect(await causes(base, head)).toEqual([
      'the base asset bytes do not match the ledgered sha256'
    ])
  })

  it.each([
    ['a traversal', `${ASSETS}/../page/First.tsx`],
    ['a path outside the assets folder', `${GEN}/page/First.tsx`]
  ])('should reject %s', async (_, file) => {
    await forge(
      head,
      [
        { path: orphan, sha256: sha256(await readFile(path.join(base, orphan))) },
        { path: file, sha256: 'c'.repeat(64) }
      ].sort((a, b) => (a.path < b.path ? -1 : 1))
    )
    expect(await causes(base, head)).toContain('ledger entry is not a generated asset path')
  })

  it('should reject a symlink that escapes the tree', async () => {
    const outside = await tempDir('orphan-outside-')
    await writeFile(path.join(outside, 'x.svg'), '<svg/>')
    const link = `${ASSETS}/eeeeeeeeeeeeeeee.svg`
    await symlink(path.join(outside, 'x.svg'), path.join(head, link))
    await forge(head, [
      { path: link, sha256: sha256(Buffer.from('<svg/>')) },
      { path: orphan, sha256: sha256(await readFile(path.join(base, orphan))) }
    ])
    expect((await causes(base, head)).join('\n')).toContain('not contained')
  })
})

describe('when the ledger is a registered ownership aggregate', () => {
  let head: string
  let orphan: string
  beforeEach(async () => {
    ;({ head, orphan } = await scenario())
  })
  const unowned = async (root: string) => {
    const ownership = await buildOwnership(root)
    const files: string[] = []
    for (const directory of GUARDED_DIRECTORIES)
      for (const file of await listFiles(path.join(root, directory)))
        files.push(toPosix(path.relative(root, file)))
    return files.filter((file) => ownerOf(ownership, file) === null).sort()
  }
  // The automated guard over the snapshot and generated output, as the sync runs it.
  const guard = (baseDir: string, stagedDir: string) =>
    checkScope({
      baseDir,
      stagedDir,
      capturedPageIds: ['1:1'],
      runKind: 'automated',
      stage: 'complete',
      retirements: emptyRetirements(),
      directories: [SNAP, GEN],
      files: []
    })
  // Every finding about the ledger file must come from checkLedger, never from aggregate handling.
  const ledgerFindings = async (baseDir: string, stagedDir: string) => {
    const { violations } = await guard(baseDir, stagedDir)
    expect(
      violations.filter((item) => item.path === LEDGER && item.kind !== 'orphan-ledger')
    ).toEqual([])
    return violations.filter((item) => item.kind === 'orphan-ledger').map((item) => item.cause)
  }

  it('should own a populated ledger, adding nothing unowned to the tree', async () => {
    expect((await loadLedger(head)).orphaned).toHaveLength(1)
    const base = await copyTree(head, 'orphan-no-ledger-')
    await rm(path.join(base, LEDGER))

    expect(ownerOf(await buildOwnership(head), LEDGER)).toEqual({ aggregate: 'orphan-ledger' })
    expect(await unowned(head)).toEqual(await unowned(base))
  })

  it('should leave an unchanged populated ledger without findings', async () => {
    expect(await ledgerFindings(head, await copyTree(head, 'orphan-same-'))).toEqual([])
  })

  it('should reject a malformed ledger', async () => {
    const next = await copyTree(head, 'orphan-malformed-')
    await put(next, LEDGER, JSON.stringify({ schema: 1, orphaned: 'all' }))
    expect(await ledgerFindings(head, next)).toHaveLength(1)
  })

  it('should reject a forged addition', async () => {
    const next = await copyTree(head, 'orphan-forged-')
    const stray = `${ASSETS}/ffffffffffffffff.svg`
    await put(
      next,
      LEDGER,
      serializeLedger([
        ...(await loadLedger(head)).orphaned,
        { path: stray, sha256: 'c'.repeat(64) }
      ])
    )
    expect((await ledgerFindings(head, next)).length).toBeGreaterThan(0)
  })

  it('should reject an edit of an existing entry', async () => {
    const next = await copyTree(head, 'orphan-edit-')
    await put(next, LEDGER, serializeLedger([{ path: orphan, sha256: 'a'.repeat(64) }]))
    expect(await ledgerFindings(head, next)).toEqual(['an existing ledger entry was edited'])
  })

  it('should reject an automated removal of an entry and its asset', async () => {
    const next = await copyTree(head, 'orphan-remove-')
    await put(next, LEDGER, serializeLedger([]))
    await rm(path.join(next, orphan))
    expect(await ledgerFindings(head, next)).toEqual([
      'only a developer reactivation or drain may remove a ledger entry'
    ])
  })
})

describe('when a developer reactivates or drains a ledgered asset', () => {
  let head: string
  let orphan: string
  let committed: string
  beforeEach(async () => {
    ;({ head, orphan } = await scenario())
    // The tree with the ledger as committed: the base of the developer PR.
    committed = await copyTree(head, 'orphan-committed-')
    execFileSync('git', ['init', '-q'], { cwd: head })
    execFileSync(
      'git',
      ['-c', 'user.email=fixture@example.com', '-c', 'user.name=Fixture', 'add', '-A'],
      { cwd: head }
    )
    execFileSync(
      'git',
      [
        '-c',
        'user.email=fixture@example.com',
        '-c',
        'user.name=Fixture',
        '-c',
        'commit.gpgsign=false',
        'commit',
        '-q',
        '-m',
        'base'
      ],
      { cwd: head }
    )
  })
  const drain = async (root: string) => {
    const app = await createApp(path.join(root, 'figma'))
    return runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
  }

  it('should accept a reactivation that keeps the bytes and has a consumer', async () => {
    await put(
      head,
      'figma/review/src/Hero.tsx',
      `import a from './generated/components/assets/${path.basename(orphan)}'`
    )
    await put(head, LEDGER, serializeLedger([]))
    expect(await causes(committed, head, true)).toEqual([])
  })

  it('should reject a reactivation without a consumer', async () => {
    await put(head, LEDGER, serializeLedger([]))
    expect(await causes(committed, head, true)).toEqual([
      'a reactivation needs the ledgered bytes and at least one consumer'
    ])
  })

  it('should reactivate through the developer command when the asset is used again', async () => {
    await put(
      head,
      'figma/review/src/Hero.tsx',
      `import a from './generated/components/assets/${path.basename(orphan)}'`
    )
    const report = (await drain(head)) as { reactivated?: string[] }
    expect(report.reactivated).toEqual([orphan])
    expect((await loadLedger(head)).orphaned).toEqual([])
    await expect(readFile(path.join(head, orphan))).resolves.toBeDefined()
  })

  it.each([
    [
      'a root document',
      'docs/art.md',
      (name: string) => `See figma/review/src/generated/components/assets/${name}`
    ],
    [
      'a root script',
      'scripts/use.mjs',
      (name: string) => `read('../figma/review/src/generated/components/assets/${name}')`
    ]
  ])('should keep an asset %s outside the staged roots still uses', async (_, file, content) => {
    await put(head, file, content(path.basename(orphan)))
    const report = (await drain(head)) as { drained?: string[]; reactivated?: string[] }
    expect(report.drained).toBeUndefined()
    expect(report.reactivated).toEqual([orphan])
    await expect(readFile(path.join(head, orphan))).resolves.toBeDefined()
  })

  it('should never delete an asset whose entry exists only in the working ledger', async () => {
    const stray = `${ASSETS}/dddddddddddddddd.svg`
    await put(head, stray, '<svg/>')
    const committedEntries = (await loadLedger(head)).orphaned
    await put(
      head,
      LEDGER,
      serializeLedger([...committedEntries, { path: stray, sha256: sha256(Buffer.from('<svg/>')) }])
    )
    await drain(head).catch(() => undefined)
    await expect(readFile(path.join(head, stray), 'utf8')).resolves.toBe('<svg/>')
  })

  it('should drain file and entry together, valid against the base ledger', async () => {
    const report = (await drain(head)) as { drained?: string[] }
    expect(report.drained).toEqual([orphan])
    await expect(readFile(path.join(head, orphan))).rejects.toMatchObject({ code: 'ENOENT' })
    expect((await loadLedger(head)).orphaned).toEqual([])
    const check = await ledgerCheck(committed, head, true)
    expect(check.violations).toEqual([])
    expect([...check.drained]).toEqual([orphan])
  })

  it('should complete a drain interrupted after deleting the file', async () => {
    await rm(path.join(head, orphan))
    await drain(head)
    expect((await loadLedger(head)).orphaned).toEqual([])
    expect(await causes(committed, head, true)).toEqual([])
  })

  it('should complete a drain interrupted after dropping the entry', async () => {
    await put(head, LEDGER, serializeLedger([]))
    await drain(head)
    await expect(readFile(path.join(head, orphan))).rejects.toMatchObject({ code: 'ENOENT' })
    expect(await causes(committed, head, true)).toEqual([])
  })

  it('should be a no-op when repeated', async () => {
    await drain(head)
    const before = await fixtureBytes(head)
    await drain(head)
    expect(await fixtureBytes(head)).toEqual(before)
  })

  it('should reject a deleted ledgered file that keeps its entry', async () => {
    await rm(path.join(head, orphan))
    expect(await causes(committed, head, true)).toEqual([
      'a ledgered asset was deleted without removing its entry'
    ])
  })
})

// The CLI exit status, run as CI runs it: `npm run cli -- diff` from the package directory.
const CLI = path.resolve(import.meta.dirname, '../cli.ts')
const TSX = path.resolve(import.meta.dirname, '../../../node_modules/.bin/tsx')
const cliDiff = (root: string) =>
  spawnSync(TSX, [CLI, 'diff'], { cwd: path.join(root, 'figma'), encoding: 'utf8' }).status

describe('when cli diff checks a tree with ledgered orphans', () => {
  let head: string
  let orphan: string
  beforeEach(async () => {
    ;({ head, orphan } = await scenario())
  })

  it('should exit 0 with valid retained entries alone', () => {
    expect(cliDiff(head)).toBe(0)
  })

  it('should exit 1 on a ledger issue', async () => {
    await put(
      head,
      'figma/review/src/Hero.tsx',
      `import a from './generated/components/assets/${path.basename(orphan)}'`
    )
    expect(cliDiff(head)).toBe(1)
  })

  it('should exit 1 on added drift', async () => {
    await rm(path.join(head, GEN, 'page/Second.tsx'))
    expect(cliDiff(head)).toBe(1)
  })

  it('should exit 1 on changed drift', async () => {
    await writeFile(path.join(head, GEN, 'page/Second.tsx'), '// edited by hand\n')
    expect(cliDiff(head)).toBe(1)
  })

  it('should exit 1 on removed drift beside a retained orphan', async () => {
    await put(head, `${GEN}/page/Stray.tsx`, '// not generated\n')
    expect(cliDiff(head)).toBe(1)
  })
})
