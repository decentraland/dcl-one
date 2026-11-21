import { cp, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { checkScope, type RunKind, type StageCompleteness } from './check-scope.js'
import { applyRetirements, computeOwned } from './apply-retirements.js'
import { emptyRetirements, type Retirements } from './retirements.js'
import {
  BUTTONS,
  GEN,
  PLACES,
  REVIEW,
  SNAP,
  components,
  index,
  manifest,
  put,
  writeTree
} from './test-fixtures.js'

describe('checkScope', () => {
  let base: string
  let staged: string

  beforeEach(async () => {
    base = await mkdtemp(path.join(os.tmpdir(), 'dcl-scope-base-'))
    staged = await mkdtemp(path.join(os.tmpdir(), 'dcl-scope-staged-'))
    await writeTree(base)
    await cp(base, staged, { recursive: true })
  })

  afterEach(async () => {
    await rm(base, { recursive: true, force: true })
    await rm(staged, { recursive: true, force: true })
  })

  const run = (
    options: {
      captured?: string[]
      runKind?: RunKind
      retirements?: Retirements
      foundationsCaptured?: boolean
      removalsOnly?: string[]
      directories?: string[]
      files?: string[]
      stage?: StageCompleteness
    } = {}
  ) =>
    checkScope({
      baseDir: base,
      stagedDir: staged,
      capturedPageIds: options.captured ?? [BUTTONS],
      runKind: options.runKind ?? 'automated',
      stage: options.stage ?? 'complete',
      retirements: options.retirements ?? emptyRetirements(),
      foundationsCaptured: options.foundationsCaptured,
      removalsOnly: options.removalsOnly,
      directories: options.directories,
      files: options.files
    })

  const kinds = async (options?: Parameters<typeof run>[0]) =>
    (await run(options)).violations.map((violation) => `${violation.kind}:${violation.path}`)

  describe('when shared tokens affect a frozen page', () => {
    beforeEach(async () => {
      await put(base, `${GEN}/tokens/tokens.css`, ':root { --color: red; --alias: var(--color); }')
      await put(
        staged,
        `${GEN}/tokens/tokens.css`,
        ':root { --color: blue; --alias: var(--color); }'
      )
      await put(
        base,
        `${GEN}/components/places/PlaceCard.module.css`,
        '.card { color: var(--alias); }'
      )
      await put(
        staged,
        `${GEN}/components/places/PlaceCard.module.css`,
        '.card { color: var(--alias); }'
      )
    })
    it('should reject the indirect change although the component bytes are unchanged', async () => {
      expect((await run({ foundationsCaptured: true })).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: `${GEN}/components/places/PlaceCard.module.css`,
            kind: 'out-of-scope-change'
          })
        ])
      )
    })
    describe('and the dependent page is captured', () => {
      it('should allow the change', async () => {
        expect((await run({ captured: [BUTTONS, PLACES], foundationsCaptured: true })).ok).toBe(
          true
        )
      })
    })
    describe('and the run is a developer run', () => {
      it('should allow an explicit developer change', async () => {
        expect((await run({ runKind: 'developer' })).ok).toBe(true)
      })
    })
  })

  describe('when retirement consistency changes', () => {
    describe('and a permanent identity is removed', () => {
      let record: Retirements

      beforeEach(async () => {
        record = {
          schema: 1,
          retired: [
            {
              pageId: 'gone',
              name: 'Gone',
              reason: 'Removed',
              owned: [],
              sharedExclusions: []
            }
          ]
        }
        await put(base, `figma/ledger/retirements.json`, record)
      })

      it('should reject the removal even in developer mode', async () => {
        expect((await run({ runKind: 'developer' })).violations).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              kind: 'retirement-edit',
              cause: expect.stringContaining('permanent')
            })
          ])
        )
      })
    })

    describe('and a developer PR adds a pending record', () => {
      beforeEach(async () => {
        await put(staged, `figma/ledger/retirements.json`, {
          schema: 1,
          retired: [
            {
              pageId: BUTTONS,
              name: 'Buttons',
              reason: 'Removed',
              owned: [`${GEN}/components/buttons/Button.tsx`],
              sharedExclusions: []
            }
          ]
        })
      })

      it('should reject the newly added pending record', async () => {
        expect(
          (await run({ runKind: 'developer' })).violations.some(
            (violation) => violation.kind === 'pending-retirement'
          )
        ).toBe(true)
      })
    })

    describe('and a seal carries an incorrect input hash', () => {
      beforeEach(async () => {
        await put(staged, `${REVIEW}/promotion.json`, {
          schema: 2,
          components: [],
          excluded: [],
          files: { [`${GEN}/components/buttons/Button.tsx`]: 'forged' }
        })
      })

      it('should reject the seal', async () => {
        expect((await run()).violations).toEqual(
          expect.arrayContaining([
            expect.objectContaining({ cause: expect.stringContaining('fingerprint') })
          ])
        )
      })
    })
  })

  describe('when a selected inventory page expands beyond its exception', () => {
    beforeEach(async () => {
      await put(base, `${SNAP}/generation-inventory.json`, {
        schema: 1,
        review: [{ pageId: BUTTONS, componentIds: ['b1'] }],
        promotion: []
      })
      await put(staged, `${SNAP}/generation-inventory.json`, {
        schema: 1,
        review: [{ pageId: BUTTONS }],
        promotion: []
      })
    })

    it('should retain implicit membership without treating expansion as removal', async () => {
      expect((await run()).ok).toBe(true)
    })
  })

  describe('when a new shared asset is staged', () => {
    describe('and only a selected component uses it', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/assets/selected.svg`, '<svg/>')
        await put(
          staged,
          `${GEN}/components/buttons/Button.tsx`,
          "import artwork from '../assets/selected.svg'"
        )
      })

      it('should allow the asset', async () => {
        expect((await run()).ok).toBe(true)
      })
    })

    describe('and nothing owns it', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/assets/orphan.svg`, '<svg/>')
      })

      it('should reject the unowned asset', async () => {
        expect((await run()).violations).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              path: `${GEN}/components/assets/orphan.svg`,
              cause: expect.stringContaining('unowned')
            })
          ])
        )
      })
    })

    describe('and a frozen component still uses the restored asset', () => {
      beforeEach(async () => {
        for (const root of [base, staged])
          await put(
            root,
            `${GEN}/components/places/PlaceCard.tsx`,
            "import artwork from '../assets/shared.svg'"
          )
        await put(
          staged,
          `${GEN}/components/buttons/Button.tsx`,
          "import artwork from '../assets/shared.svg'"
        )
        await put(staged, `${GEN}/components/assets/shared.svg`, '<svg/>')
      })

      it('should reject the restored asset', async () => {
        expect((await run()).violations).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              path: `${GEN}/components/assets/shared.svg`,
              cause: expect.stringContaining('unselected')
            })
          ])
        )
      })
    })

    describe('and all its users were captured', () => {
      beforeEach(async () => {
        await put(
          staged,
          `${GEN}/components/buttons/Button.tsx`,
          "import artwork from '../assets/shared.svg'"
        )
        await put(
          staged,
          `${GEN}/components/places/PlaceCard.tsx`,
          "import artwork from '../assets/shared.svg'"
        )
        await put(staged, `${GEN}/components/assets/shared.svg`, '<svg/>')
      })

      it('should allow the shared asset', async () => {
        expect((await run({ captured: [BUTTONS, PLACES] })).ok).toBe(true)
      })
    })

    describe('and foreign code consumes it', () => {
      beforeEach(async () => {
        for (const root of [base, staged])
          await put(
            root,
            'figma/review/src/custom/Keep.tsx',
            "import artwork from '../generated/components/assets/shared.svg'"
          )
        await put(staged, `${GEN}/components/assets/shared.svg`, '<svg/>')
      })

      it('should reject the asset', async () => {
        expect((await run({ captured: [BUTTONS, PLACES] })).ok).toBe(false)
      })
    })

    describe('and an existing shared asset changes bytes', () => {
      beforeEach(async () => {
        for (const root of [base, staged]) {
          await put(
            root,
            `${GEN}/components/buttons/Button.tsx`,
            "import artwork from '../assets/shared.svg'"
          )
          await put(root, `${GEN}/components/assets/shared.svg`, '<svg/>')
        }
        await put(staged, `${GEN}/components/assets/shared.svg`, '<svg>changed</svg>')
      })

      it('should keep existing shared asset bytes immutable', async () => {
        expect((await run({ captured: [BUTTONS, PLACES] })).violations).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              path: `${GEN}/components/assets/shared.svg`,
              cause: expect.stringContaining('bytes must remain unchanged')
            })
          ])
        )
      })
    })
  })
  describe('when a public component name is reused for another ID', () => {
    beforeEach(async () => {
      const next = components.map((component) =>
        component.name === 'Button' ? { ...component, id: 'replacement' } : component
      )
      await put(staged, `${GEN}/components/manifest.ts`, manifest(next))
      await put(staged, `${REVIEW}/catalog.json`, {
        components: next.map(({ name, page, id, variants }) => ({ name, page, id, variants })),
        pages: [
          { id: BUTTONS, name: 'Buttons' },
          { id: PLACES, name: 'Places' }
        ],
        foundations: { tokens: [] }
      })
    })

    it('should reject identity replacement even on a selected page', async () => {
      expect((await run()).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: 'ownership',
            cause: expect.stringContaining('componentIdByName')
          })
        ])
      )
    })
  })

  describe('when nothing changed', () => {
    it('should pass', async () => {
      expect(await run()).toEqual({ ok: true, violations: [] })
    })
  })

  describe('when more files change than one concurrent read window holds', () => {
    const extras = Array.from(
      { length: 70 },
      (_, i) => `${GEN}/components/places/Extra${String(i).padStart(2, '0')}.tsx`
    )

    beforeEach(async () => {
      for (const file of extras) await put(staged, file, 'extra')
    })

    it('should report every file once and in path order', async () => {
      expect(await kinds()).toEqual(extras.map((file) => `out-of-scope-change:${file}`))
    })

    describe('and one read in a window is rejected', () => {
      beforeEach(async () => {
        const outside = path.join(base, 'outside.tsx')
        await writeFile(outside, 'outside')
        await rm(path.join(staged, extras[40]!))
        await symlink(outside, path.join(staged, extras[40]!))
      })

      it('should fail closed', async () => {
        await expect(run()).rejects.toThrow(/symlink/i)
      })
    })
  })

  describe('when a captured page updates its own artifacts', () => {
    describe('and it updates its files and its entries in the shared aggregates', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/buttons/Button.tsx`, 'button v2')
        await put(staged, `${GEN}/components/buttons/Button.module.css`, 'css v2')
        await put(staged, `${SNAP}/components/Buttons.json`, {
          v: 2,
          page: { id: BUTTONS, name: 'Buttons' },
          components: [{ id: 'b1', name: 'Button', base: { id: '10:1' }, others: [] }]
        })
        const next = components.map((c) => (c.name === 'Button' ? { ...c, notes: ['updated'] } : c))
        await put(staged, `${GEN}/components/manifest.ts`, manifest(next))
        await put(staged, `${REVIEW}/catalog.json`, {
          components: next.map(({ name, page, id, variants }) => ({ name, page, id, variants })),
          pages: [
            { id: BUTTONS, name: 'Buttons' },
            { id: PLACES, name: 'Places' }
          ],
          foundations: { tokens: [] }
        })
      })

      it('should allow its files and its entries in the shared aggregates', async () => {
        expect(await run()).toEqual({ ok: true, violations: [] })
      })
    })

    describe('and it adds a new component', () => {
      beforeEach(async () => {
        const added = {
          name: 'Chip',
          page: 'Buttons',
          id: 'c1',
          file: 'buttons/Chip',
          variants: []
        }
        await put(staged, `${SNAP}/components/Buttons.json`, {
          v: 1,
          page: { id: BUTTONS, name: 'Buttons' },
          components: [
            { id: 'b1', name: 'Button', base: { id: '10:1' }, others: [] },
            { id: 'c1', name: 'Chip', others: [] }
          ]
        })
        await put(staged, `${GEN}/components/buttons/Chip.tsx`, 'chip')
        await put(staged, `${GEN}/components/manifest.ts`, manifest([...components, added]))
        await put(staged, `${GEN}/components/index.ts`, index([...components, added]))
      })

      it('should allow a new component on the captured page', async () => {
        expect(await run()).toEqual({ ok: true, violations: [] })
      })
    })
  })

  describe('when an unselected page changes', () => {
    describe('and its component file changes', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')
      })

      it('should reject its component file', async () => {
        expect(await kinds()).toEqual([
          `out-of-scope-change:${GEN}/components/places/PlaceCard.tsx`
        ])
      })
    })

    describe('and its snapshot, render and reference image change', () => {
      beforeEach(async () => {
        await put(staged, `${SNAP}/components/Places.json`, {
          v: 2,
          page: { id: PLACES, name: 'Places' },
          components: []
        })
        await put(staged, 'figma/review/public/dcl-ui-review/renders/PlaceCard-0.png', 'png2')
        await put(staged, 'figma/review/public/dcl-ui-review/references/live-20-1.png', 'png2')
      })

      it('should reject them as out-of-scope changes', async () => {
        expect((await kinds()).every((entry) => entry.startsWith('out-of-scope-change'))).toBe(true)
      })

      it('should reject each of the three', async () => {
        expect(await kinds()).toHaveLength(3)
      })
    })

    describe('and its entry in a shared aggregate changes', () => {
      beforeEach(async () => {
        const catalog = JSON.parse(
          await readFile(path.join(staged, `${REVIEW}/catalog.json`), 'utf8')
        )
        catalog.components.find((entry: { name: string }) => entry.name === 'PlaceCard').variants =
          ['changed']
        await put(staged, `${REVIEW}/catalog.json`, catalog)
      })

      it('should reject the changed unselected entry', async () => {
        expect(await kinds()).toEqual([`aggregate-entry-changed:${REVIEW}/catalog.json`])
      })
    })

    describe('and the catalog of its entries is reordered', () => {
      beforeEach(async () => {
        const more = [
          ...components,
          { name: 'Zeta', page: 'Places', id: 'z', file: 'places/Zeta', variants: [] }
        ]
        await put(base, `${GEN}/components/manifest.ts`, manifest(more))
        await put(staged, `${GEN}/components/manifest.ts`, manifest([more[0]!, more[2]!, more[1]!]))
      })

      it('should reject the reordered catalog', async () => {
        expect((await run()).violations.map((v) => v.cause).join(' ')).toContain('order')
      })
    })
  })

  describe('when something is removed', () => {
    describe('and it is a file of a captured page', () => {
      beforeEach(async () => {
        await rm(path.join(staged, `${GEN}/components/buttons/Button.module.css`))
      })

      it('should reject the removed file', async () => {
        expect(await kinds()).toEqual([`removed:${GEN}/components/buttons/Button.module.css`])
      })
    })

    describe('and it is an entry in a shared aggregate', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/manifest.ts`, manifest([components[0]!]))
      })

      it('should reject the removed entry', async () => {
        expect((await kinds()).some((entry) => entry.startsWith('removed:'))).toBe(true)
      })
    })

    describe('and it is a snapshot page', () => {
      beforeEach(async () => {
        await rm(path.join(staged, `${SNAP}/components/Places.json`))
      })

      it('should reject the removed snapshot page', async () => {
        expect(await kinds()).toEqual([`removed:${SNAP}/components/Places.json`])
      })
    })
  })

  describe('when a developer retires output', () => {
    const retire = (owned: string[], componentId?: string): Retirements => ({
      schema: 1,
      retired: [
        {
          pageId: PLACES,
          componentId,
          name: 'Places',
          reason: 'gone',
          owned,
          sharedExclusions: []
        }
      ]
    })

    // What a developer's retirement PR looks like once `sync --developer --apply-retirements` has run.
    async function applyCleanup(owned: string[]) {
      await applyRetirements(staged, retire(owned))
      const left = components.filter((c) => c.page !== 'Places')
      await put(staged, `${GEN}/components/manifest.ts`, manifest(left))
      await put(staged, `${GEN}/components/index.ts`, index(left))
      await put(staged, `${REVIEW}/catalog.json`, {
        components: left.map(({ name, page, id, variants }) => ({ name, page, id, variants })),
        pages: [
          { id: BUTTONS, name: 'Buttons' },
          { id: PLACES, name: 'Places' }
        ],
        foundations: { tokens: [] }
      })
      await put(staged, `${REVIEW}/references.json`, { 'Button-0': { file: '/a.png' } })
    }

    let owned: string[]

    describe('and the cleanup removes exactly what the record owns', () => {
      beforeEach(async () => {
        owned = await computeOwned(base, { pageId: PLACES })
        await applyCleanup(owned)
      })

      it('should allow the removal', async () => {
        expect(await run({ runKind: 'developer', retirements: retire(owned) })).toEqual({
          ok: true,
          violations: []
        })
      })
    })

    describe('and no record owns a removal', () => {
      const file = `${GEN}/components/places/PlaceCard.tsx`

      beforeEach(async () => {
        await rm(path.join(staged, file))
      })

      it('should still reject the removal', async () => {
        expect(await kinds({ runKind: 'developer' })).toEqual([`removed:${file}`])
      })
    })

    describe('and a frozen page changes', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')
      })

      it('should allow the developer change', async () => {
        expect(await run({ runKind: 'developer' })).toEqual({ ok: true, violations: [] })
      })
    })

    describe("and the record's output is still present", () => {
      let found: Awaited<ReturnType<typeof run>>['violations']

      beforeEach(async () => {
        owned = await computeOwned(base, { pageId: PLACES })

        found = (await run({ runKind: 'developer', retirements: retire(owned) })).violations
      })

      it('should report only pending retirements', () => {
        expect(found.every((v) => v.kind === 'pending-retirement')).toBe(true)
      })

      it('should report the still present output', () => {
        expect(found.map((v) => v.path)).toContain(`${GEN}/components/places/PlaceCard.tsx`)
      })

      it('should say the output is still listed', () => {
        expect(found.map((v) => v.cause).join(' ')).toContain('still listed')
      })
    })

    describe('and the record retires a single component', () => {
      let causes: string

      beforeEach(async () => {
        owned = await computeOwned(base, { pageId: PLACES, componentId: 'p1' })

        causes = (await run({ runKind: 'developer', retirements: retire(owned, 'p1') })).violations
          .map((v) => v.cause)
          .join(' ')
      })

      it('should flag the retired component', () => {
        expect(causes).toContain('PlaceCard')
      })

      it('should flag it by its own identity only', () => {
        expect(causes).not.toContain('Button')
      })
    })
  })

  describe('when automation touches retirements', () => {
    beforeEach(async () => {
      await put(staged, `figma/ledger/retirements.json`, {
        ...emptyRetirements(),
        retired: [{ pageId: BUTTONS, name: 'x', reason: 'y', owned: [], sharedExclusions: [] }]
      })
    })

    it('should reject an edit to the retirements file', async () => {
      expect(await kinds()).toContain(`retirement-edit:figma/ledger/retirements.json`)
    })
  })

  describe('when a shared input changes', () => {
    beforeEach(async () => {
      await put(staged, `${GEN}/tokens/tokens.css`, 'tokens v2')
    })

    describe('and no foundation page was captured', () => {
      it('should reject the tokens', async () => {
        expect(await kinds()).toEqual([`out-of-scope-change:${GEN}/tokens/tokens.css`])
      })
    })

    describe('and a foundation page was captured', () => {
      it('should allow the tokens', async () => {
        expect(await run({ foundationsCaptured: true })).toEqual({ ok: true, violations: [] })
      })
    })
  })

  describe('when an artifact has no owner or cannot be parsed', () => {
    describe('and a generated file has no owner', () => {
      beforeEach(async () => {
        await put(staged, `${GEN}/components/mystery/Thing.tsx`, 'x')
      })

      it('should reject the unowned generated file', async () => {
        expect(await kinds()).toEqual([`out-of-scope-change:${GEN}/components/mystery/Thing.tsx`])
      })
    })

    describe('and an aggregate cannot be read', () => {
      beforeEach(async () => {
        await put(staged, `${REVIEW}/catalog.json`, '{ not json')
      })

      it('should report the unreadable aggregate instead of passing it', async () => {
        expect((await run()).violations[0]?.kind).toBe('unreadable')
      })
    })
  })

  describe('when a run tries to take over or fake ownership', () => {
    describe('and it remaps a frozen component to a captured page', () => {
      beforeEach(async () => {
        const moved = components.map((c) =>
          c.name === 'PlaceCard' ? { ...c, page: 'Buttons' } : c
        )
        await put(staged, `${GEN}/components/manifest.ts`, manifest(moved))
        await put(staged, `${REVIEW}/catalog.json`, {
          components: moved.map(({ name, page, id, variants }) => ({ name, page, id, variants })),
          pages: [
            { id: BUTTONS, name: 'Buttons' },
            { id: PLACES, name: 'Places' }
          ],
          foundations: { tokens: [] }
        })
        await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')
      })

      it('should reject the change to the frozen component', async () => {
        expect(await kinds()).toContain(
          `out-of-scope-change:${GEN}/components/places/PlaceCard.tsx`
        )
      })

      it('should keep the frozen page as its owner', async () => {
        expect(
          (await run()).violations.find((violation) => violation.path.endsWith('PlaceCard.tsx'))
            ?.owner
        ).toEqual({ pageId: PLACES, componentId: 'p1' })
      })
    })

    describe('and a manifest changes only a frozen page label', () => {
      beforeEach(async () => {
        const moved = components.map((c) =>
          c.name === 'PlaceCard' ? { ...c, page: 'Buttons' } : c
        )
        await put(staged, `${GEN}/components/manifest.ts`, manifest(moved))
      })

      it('should preserve ID ownership', async () => {
        expect((await run()).violations.map((v) => v.cause).join(' ')).toContain(
          'which was not captured'
        )
      })
    })

    describe('and an aggregate did not exist before', () => {
      beforeEach(async () => {
        await put(staged, `${SNAP}/generation-inventory.json`, {
          schema: 1,
          review: [],
          promotion: []
        })
      })

      it('should reject the created aggregate', async () => {
        expect((await run()).violations[0]?.cause).toContain('created')
      })
    })

    describe('and an aggregate holds duplicate entry keys', () => {
      beforeEach(async () => {
        await put(
          staged,
          `${GEN}/components/manifest.ts`,
          manifest([...components, components[0]!])
        )
      })

      it('should reject them instead of keeping the last one', async () => {
        expect((await run()).violations.map((v) => v.kind)).toContain('unreadable')
      })
    })
  })

  describe('when the stage completeness is declared', () => {
    const library = ['components/promotion.json', 'index.ts', 'components/Modal/Modal.tsx']
    const cliDefaults = { removalsOnly: ['components'] }

    describe('and a partial stage leaves the library out', () => {
      beforeEach(async () => {
        // The library is components/ plus its entry at the root (layout.ts).
        await rm(path.join(staged, 'components'), { recursive: true })
        await rm(path.join(staged, 'index.ts'))
      })

      it('should inherit the base library and report no library violation for unchanged inputs', async () => {
        expect(await run({ stage: 'partial', ...cliDefaults })).toEqual({
          ok: true,
          violations: []
        })
      })

      it('should report the same missing library as removals in a complete stage', async () => {
        expect(await kinds({ stage: 'complete', ...cliDefaults })).toEqual(
          expect.arrayContaining(library.map((file) => `removed:${file}`))
        )
      })

      describe('and a Review token change affects an unselected page', () => {
        beforeEach(async () => {
          await put(base, `${GEN}/tokens/tokens.css`, ':root { --color: red; }')
          await put(staged, `${GEN}/tokens/tokens.css`, ':root { --color: blue; }')
          for (const root of [base, staged])
            await put(
              root,
              `${GEN}/components/places/PlaceCard.module.css`,
              '.a { color: var(--color); }'
            )
        })

        it('should keep rejecting it', async () => {
          expect((await run({ stage: 'partial' })).violations).toEqual(
            expect.arrayContaining([
              expect.objectContaining({
                path: `${GEN}/components/places/PlaceCard.module.css`,
                kind: 'out-of-scope-change',
                cause: expect.stringContaining('shared input change affects an unselected page')
              })
            ])
          )
        })
      })
    })

    describe('and a partial stage writes library bytes', () => {
      describe('and a library file changes', () => {
        beforeEach(async () => {
          await put(staged, 'components/buttons/Button.tsx', 'new')
        })

        it('should reject the staged library change', async () => {
          expect(await kinds({ stage: 'partial', captured: [BUTTONS, PLACES] })).toEqual([
            'out-of-scope-change:components/buttons/Button.tsx'
          ])
        })
      })

      describe('and a library file is added', () => {
        beforeEach(async () => {
          await put(staged, 'components/buttons/Extra.tsx', 'new')
        })

        it('should reject the staged library addition', async () => {
          expect(await kinds({ stage: 'partial', captured: [BUTTONS, PLACES] })).toEqual([
            'out-of-scope-change:components/buttons/Extra.tsx'
          ])
        })
      })

      describe('and library entries are removed', () => {
        beforeEach(async () => {
          await put(staged, 'components/promotion.json', {
            schema: 1,
            files: {},
            components: ['Button']
          })
        })

        it('should reject the staged removal', async () => {
          expect(await kinds({ stage: 'partial', captured: [BUTTONS, PLACES] })).toEqual([
            'out-of-scope-change:components/promotion.json'
          ])
        })
      })
    })

    describe('and a complete stage mutates the library', () => {
      describe('and it adds a promotion entry the run does not own', () => {
        beforeEach(async () => {
          await put(staged, 'components/promotion.json', {
            schema: 1,
            files: {},
            components: ['Button', 'PlaceCard', 'Unowned']
          })
        })

        it('should reject the promotion entry', async () => {
          expect(await kinds({ stage: 'complete', ...cliDefaults })).toEqual([
            'aggregate-entry-changed:components/promotion.json'
          ])
        })
      })

      describe('and it removes the library entry point', () => {
        beforeEach(async () => {
          await rm(path.join(staged, 'index.ts'))
        })

        it('should reject the removed library entry point', async () => {
          expect(await kinds({ stage: 'complete', ...cliDefaults })).toEqual(
            expect.arrayContaining(['removed:index.ts'])
          )
        })
      })
    })
  })

  describe('when only removals are watched in a folder', () => {
    beforeEach(async () => {
      await put(base, 'components/Modal/Modal.tsx', 'modal')
      await put(base, 'components/a.ts', 'a')
      await cp(path.join(base, 'components'), path.join(staged, 'components'), {
        recursive: true
      })
    })
    const watch = { removalsOnly: ['components'], directories: [] as string[] }

    describe('and a promotion adds and changes library files', () => {
      beforeEach(async () => {
        await put(staged, 'components/buttons/Button.tsx', 'button2')
        await put(staged, 'components/b.ts', 'b')
      })

      it('should allow them', async () => {
        expect(await run(watch)).toEqual({ ok: true, violations: [] })
      })
    })

    describe('and an existing unowned library file changes', () => {
      beforeEach(async () => {
        await put(staged, 'components/a.ts', 'changed')
      })

      it('should preserve it even in the narrow promotion scope', async () => {
        expect((await run(watch)).violations).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              path: 'components/a.ts',
              owner: null,
              cause: expect.stringContaining('no recorded base ownership')
            })
          ])
        )
      })
    })

    describe('and a hand-written library directory is deleted', () => {
      beforeEach(async () => {
        await rm(path.join(staged, 'components/Modal'), { recursive: true })
      })

      it('should reject the deletion', async () => {
        expect(await kinds(watch)).toEqual(['removed:components/Modal/Modal.tsx'])
      })
    })
  })

  describe('when a retirement touches the promoted library', () => {
    let retirements: Retirements

    beforeEach(async () => {
      retirements = {
        schema: 1,
        retired: [
          {
            pageId: PLACES,
            name: 'Places',
            reason: 'gone',
            owned: await computeOwned(base, { pageId: PLACES }),
            sharedExclusions: []
          }
        ]
      }
    })

    describe('and its owned paths are computed', () => {
      let owned: string[]

      beforeEach(() => {
        owned = retirements.retired[0]!.owned
      })

      it('should own the library copy, the exports and the promotion entry', () => {
        expect(owned).toEqual(
          expect.arrayContaining([
            'components/places/PlaceCard.tsx',
            'components/places/PlaceCard.figma.ts',
            'index.ts#value:PlaceCard',
            'index.ts#type:PlaceCardProps',
            'components/promotion.json#component:PlaceCard'
          ])
        )
      })

      it('should not own the live Button', () => {
        expect(owned.join(' ')).not.toContain('Button')
      })

      it('should not own the hand-written Modal', () => {
        expect(owned.join(' ')).not.toContain('Modal')
      })
    })

    describe('and the library still exports and lists the retired component', () => {
      let causes: string

      beforeEach(async () => {
        causes = (await run({ runKind: 'developer', retirements })).violations
          .map((v) => v.cause)
          .join(' ')
      })

      it('should flag the export', () => {
        expect(causes).toContain('value:PlaceCard')
      })

      it('should flag the promotion entry', () => {
        expect(causes).toContain('component:PlaceCard')
      })
    })

    describe('and what it shares with live items is missing', () => {
      beforeEach(async () => {
        retirements.retired[0]!.sharedExclusions = [`${GEN}/components/buttons/Button.tsx`]
        await rm(path.join(staged, `${GEN}/components/buttons/Button.tsx`))
      })

      it('should require it to stay', async () => {
        expect(
          (await run({ runKind: 'developer', retirements })).violations.map((v) => v.cause)
        ).toContain('shared with live items by the retirement of Places, but missing')
      })
    })
  })
})
