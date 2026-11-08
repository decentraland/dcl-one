import { cp, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { encodePageFileName } from '../capture/page-file-name.js'
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
    it('should reject the indirect change even if component bytes are unchanged', async () => {
      expect((await run({ foundationsCaptured: true })).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: `${GEN}/components/places/PlaceCard.module.css`,
            kind: 'out-of-scope-change'
          })
        ])
      )
    })
    it('should allow the change when the dependent page is captured', async () => {
      expect((await run({ captured: [BUTTONS, PLACES], foundationsCaptured: true })).ok).toBe(true)
    })
    it('should allow an explicit developer change', async () => {
      expect((await run({ runKind: 'developer' })).ok).toBe(true)
    })
  })

  describe('when retirement consistency changes', () => {
    it('should reject removal of a permanent identity even in developer mode', async () => {
      const record: Retirements = {
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
      await put(base, `${SNAP}/retirements.json`, record)
      expect((await run({ runKind: 'developer' })).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            kind: 'retirement-edit',
            cause: expect.stringContaining('permanent')
          })
        ])
      )
    })
    it('should reject a newly added pending record in a developer PR', async () => {
      await put(staged, `${SNAP}/retirements.json`, {
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
      expect(
        (await run({ runKind: 'developer' })).violations.some(
          (violation) => violation.kind === 'pending-retirement'
        )
      ).toBe(true)
    })
    it('should reject a seal with an incorrect input hash', async () => {
      await put(staged, `${REVIEW}/promotion.json`, {
        schema: 2,
        components: [],
        excluded: [],
        files: { [`${GEN}/components/buttons/Button.tsx`]: 'forged' }
      })
      expect((await run()).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ cause: expect.stringContaining('fingerprint') })
        ])
      )
    })
  })

  describe('when a selected inventory page expands beyond its exception', () => {
    it('should retain implicit membership without treating expansion as removal', async () => {
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
      expect((await run()).ok).toBe(true)
    })
  })

  describe('when a new shared asset is staged', () => {
    it('should allow an asset used only by a selected component', async () => {
      await put(staged, `${GEN}/components/assets/selected.svg`, '<svg/>')
      await put(
        staged,
        `${GEN}/components/buttons/Button.tsx`,
        "import artwork from '../assets/selected.svg'"
      )
      expect((await run()).ok).toBe(true)
    })
    it('should reject an unowned asset', async () => {
      await put(staged, `${GEN}/components/assets/orphan.svg`, '<svg/>')
      expect((await run()).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: `${GEN}/components/assets/orphan.svg`,
            cause: expect.stringContaining('unowned')
          })
        ])
      )
    })
    it('should reject a restored asset still used by a frozen component', async () => {
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
      expect((await run()).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: `${GEN}/components/assets/shared.svg`,
            cause: expect.stringContaining('unselected')
          })
        ])
      )
    })
    it('should allow a shared asset when all its users were captured', async () => {
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
      expect((await run({ captured: [BUTTONS, PLACES] })).ok).toBe(true)
    })
    it('should reject an asset consumed by foreign code', async () => {
      for (const root of [base, staged])
        await put(
          root,
          'figma/review/src/custom/Keep.tsx',
          "import artwork from '../generated/components/assets/shared.svg'"
        )
      await put(staged, `${GEN}/components/assets/shared.svg`, '<svg/>')
      expect((await run({ captured: [BUTTONS, PLACES] })).ok).toBe(false)
    })
    it('should keep existing shared asset bytes immutable', async () => {
      for (const root of [base, staged]) {
        await put(
          root,
          `${GEN}/components/buttons/Button.tsx`,
          "import artwork from '../assets/shared.svg'"
        )
        await put(root, `${GEN}/components/assets/shared.svg`, '<svg/>')
      }
      await put(staged, `${GEN}/components/assets/shared.svg`, '<svg>changed</svg>')
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
  describe('when a public component name is reused for another ID', () => {
    it('should reject identity replacement even on a selected page', async () => {
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

    it('should report every file once and in path order', async () => {
      for (const file of extras) await put(staged, file, 'extra')

      expect(await kinds()).toEqual(extras.map((file) => `out-of-scope-change:${file}`))
    })

    it('should fail closed when one read in a window is rejected', async () => {
      for (const file of extras) await put(staged, file, 'extra')
      const outside = path.join(base, 'outside.tsx')
      await writeFile(outside, 'outside')
      await rm(path.join(staged, extras[40]!))
      await symlink(outside, path.join(staged, extras[40]!))

      await expect(run()).rejects.toThrow(/symlink/i)
    })
  })

  describe('when a captured page updates its own artifacts', () => {
    it('should allow its files and its entries in the shared aggregates', async () => {
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

      expect(await run()).toEqual({ ok: true, violations: [] })
    })

    it('should allow a new component on the captured page', async () => {
      const added = { name: 'Chip', page: 'Buttons', id: 'c1', file: 'buttons/Chip', variants: [] }
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

      expect(await run()).toEqual({ ok: true, violations: [] })
    })
  })

  describe('when visual evidence is rewritten', () => {
    const FIDELITY = 'figma/review/public/dcl-ui-review/fidelity.json'
    const evidence = {
      'Button-0': { sourceId: '10:1', image: '/dcl-ui-review/renders/Button-0.png' },
      'PlaceCard-0': {
        sourceId: '20:1',
        image: '/dcl-ui-review/renders/PlaceCard-0.png'
      },
      'PlaceCard-1': { sourceId: '20:2', image: '/dcl-ui-review/renders/PlaceCard-1.png' }
    }
    const fidelity = async (cases: Record<string, unknown>) => {
      await put(base, FIDELITY, { cases: evidence })
      await put(staged, FIDELITY, { cases })
      return (await run()).violations
        .filter((violation) => violation.path === FIDELITY)
        .map((violation) => violation.cause)
    }

    it('should accept new evidence for the captured page that keeps the others in order', async () => {
      const { 'Button-0': button, ...others } = evidence
      expect(await fidelity({ ...others, 'Button-0': { ...button, status: 'different' } })).toEqual(
        []
      )
    })

    it('should reject changed evidence for a page that was not captured', async () => {
      expect(
        await fidelity({ ...evidence, 'PlaceCard-0': { ...evidence['PlaceCard-0'], status: 'x' } })
      ).toEqual([`entry PlaceCard-0 belongs to ${PLACES}, which was not captured`])
    })

    it('should reject a reorder of evidence the run did not select', async () => {
      const { 'PlaceCard-0': first, 'PlaceCard-1': second, ...rest } = evidence
      expect(await fidelity({ ...rest, 'PlaceCard-1': second, 'PlaceCard-0': first })).toEqual([
        'the order of unselected entries changed'
      ])
    })

    describe('and the staged run renames components while the evidence keeps their old names', () => {
      // Staged ownership is rebuilt from the staged manifest and catalog, so a renamed component's old
      // case keys have no staged owner even though the same root ID is still on the same page.
      const stageCatalog = async (
        list: { name: string; page: string; id: string; variants: { id: string }[] }[],
        pages: Record<string, { id: string; name: string }[]>
      ) => {
        await put(
          staged,
          `${GEN}/components/manifest.ts`,
          manifest(
            list.map((component) => ({
              ...component,
              file: `${component.page.toLowerCase()}/${component.name}`
            }))
          )
        )
        await put(staged, `${REVIEW}/catalog.json`, {
          fileKey: 'F',
          components: list,
          pages: [
            { id: BUTTONS, name: 'Buttons' },
            { id: PLACES, name: 'Places' }
          ],
          foundations: { tokens: [] }
        })
        for (const [name, id] of [
          ['Buttons', BUTTONS],
          ['Places', PLACES]
        ] as const)
          await put(staged, `${SNAP}/components/${encodePageFileName(name)}`, {
            v: 2,
            page: { id, name },
            components: (pages[name] ?? []).map((component) => ({
              id: component.id,
              name: component.name,
              others: []
            }))
          })
      }
      const renameButton = () =>
        stageCatalog(
          [
            { name: 'JumpInButton', page: 'Buttons', id: 'b1', variants: [{ id: '10:1' }] },
            { name: 'PlaceCard', page: 'Places', id: 'p1', variants: [{ id: '20:1' }] }
          ],
          {
            Buttons: [{ id: 'b1', name: 'JumpInButton' }],
            Places: [{ id: 'p1', name: 'PlaceCard' }]
          }
        )
      const causes = async (before: Record<string, unknown>, after: Record<string, unknown>) => {
        await put(base, FIDELITY, { cases: before })
        await put(staged, FIDELITY, { cases: after })
        return (await run()).violations
          .filter((violation) => violation.path === FIDELITY)
          .map((violation) => violation.cause)
      }
      const { 'Button-0': button, 'PlaceCard-0': first, 'PlaceCard-1': second } = evidence
      // The captured page re-renders its case, so the aggregate's bytes change.
      const rerendered = { ...button, status: 'different' }

      it('should accept the renamed case in its committed position', async () => {
        await renameButton()
        expect(
          await causes(evidence, {
            'Button-0': rerendered,
            'PlaceCard-0': first,
            'PlaceCard-1': second
          })
        ).toEqual([])
      })

      it('should reject a swap of the uncaptured entries', async () => {
        await renameButton()
        expect(
          await causes(evidence, {
            'Button-0': button,
            'PlaceCard-1': second,
            'PlaceCard-0': first
          })
        ).toEqual(['the order of unselected entries changed'])
      })

      it('should accept the renamed case moved after the uncaptured entries', async () => {
        await renameButton()
        expect(
          await causes(evidence, {
            'PlaceCard-0': first,
            'PlaceCard-1': second,
            'Button-0': rerendered
          })
        ).toEqual([])
      })

      it('should reject a reorder of an uncaptured page that has no staged owner', async () => {
        await stageCatalog(
          [
            { name: 'Button', page: 'Buttons', id: 'b1', variants: [{ id: '10:1' }] },
            { name: 'PlaceCardNew', page: 'Places', id: 'p1', variants: [{ id: '20:1' }] }
          ],
          {
            Buttons: [{ id: 'b1', name: 'Button' }],
            Places: [{ id: 'p1', name: 'PlaceCardNew' }]
          }
        )
        expect(
          await causes(evidence, {
            'Button-0': button,
            'PlaceCard-1': second,
            'PlaceCard-0': first
          })
        ).toEqual(['the order of unselected entries changed'])
      })

      it('should keep an entry with no committed owner frozen when the staged run captures it', async () => {
        await stageCatalog(
          [
            { name: 'Button', page: 'Buttons', id: 'b1', variants: [{ id: '10:1' }] },
            { name: 'Chip', page: 'Buttons', id: 'c1', variants: [{ id: '10:2' }] },
            { name: 'PlaceCard', page: 'Places', id: 'p1', variants: [{ id: '20:1' }] }
          ],
          {
            Buttons: [
              { id: 'b1', name: 'Button' },
              { id: 'c1', name: 'Chip' }
            ],
            Places: [{ id: 'p1', name: 'PlaceCard' }]
          }
        )
        const chip = { sourceId: '10:2', image: '/dcl-ui-review/renders/Chip-0.png' }
        expect(
          await causes(
            { 'Chip-0': chip, ...evidence },
            { 'Button-0': button, 'PlaceCard-0': first, 'Chip-0': chip, 'PlaceCard-1': second }
          )
        ).toEqual(['the order of unselected entries changed'])
      })

      it('should keep an uncaptured committed owner frozen when the staged run reassigns it', async () => {
        await stageCatalog(
          [
            { name: 'Button', page: 'Buttons', id: 'b1', variants: [{ id: '10:1' }] },
            { name: 'PlaceCard', page: 'Places', id: 'p1', variants: [{ id: '20:1' }] }
          ],
          {
            Buttons: [
              { id: 'b1', name: 'Button' },
              { id: 'p1', name: 'PlaceCard' }
            ]
          }
        )
        expect(
          await causes(evidence, {
            'Button-0': button,
            'PlaceCard-1': second,
            'PlaceCard-0': first
          })
        ).toEqual(['the order of unselected entries changed'])
      })
    })
  })

  describe('when an unselected page changes', () => {
    it('should reject its component file', async () => {
      await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')

      expect(await kinds()).toEqual([`out-of-scope-change:${GEN}/components/places/PlaceCard.tsx`])
    })

    it('should reject its snapshot, render and reference image', async () => {
      await put(staged, `${SNAP}/components/Places.json`, {
        v: 2,
        page: { id: PLACES, name: 'Places' },
        components: []
      })
      await put(staged, 'figma/review/public/dcl-ui-review/renders/PlaceCard-0.png', 'png2')
      await put(staged, 'figma/review/public/dcl-ui-review/references/live-20-1.png', 'png2')

      expect((await kinds()).every((entry) => entry.startsWith('out-of-scope-change'))).toBe(true)
      expect(await kinds()).toHaveLength(3)
    })

    it('should reject a changed unselected entry in a shared aggregate', async () => {
      const refs = JSON.parse(
        await readFile(path.join(staged, `${REVIEW}/references.json`), 'utf8')
      )
      refs['PlaceCard-0'].file = '/changed.png'
      await put(staged, `${REVIEW}/references.json`, refs)

      expect(await kinds()).toEqual([`aggregate-entry-changed:${REVIEW}/references.json`])
    })

    it('should reject a reordered catalog of unselected entries', async () => {
      const more = [
        ...components,
        { name: 'Zeta', page: 'Places', id: 'z', file: 'places/Zeta', variants: [] }
      ]
      await put(base, `${GEN}/components/manifest.ts`, manifest(more))
      await put(staged, `${GEN}/components/manifest.ts`, manifest([more[0]!, more[2]!, more[1]!]))

      expect((await run()).violations.map((v) => v.cause).join(' ')).toContain('order')
    })
  })

  describe('when something is removed', () => {
    it('should reject a removed file even of a captured page', async () => {
      await rm(path.join(staged, `${GEN}/components/buttons/Button.module.css`))

      expect(await kinds()).toEqual([`removed:${GEN}/components/buttons/Button.module.css`])
    })

    it('should reject a removed entry in a shared aggregate', async () => {
      await put(staged, `${GEN}/components/manifest.ts`, manifest([components[0]!]))

      expect((await kinds()).some((entry) => entry.startsWith('removed:'))).toBe(true)
    })

    it('should reject a removed snapshot page', async () => {
      await rm(path.join(staged, `${SNAP}/components/Places.json`))

      expect(await kinds()).toEqual([`removed:${SNAP}/components/Places.json`])
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

    it('should allow removing exactly what the record owns', async () => {
      const owned = await computeOwned(base, { pageId: PLACES })
      await applyCleanup(owned)

      expect(await run({ runKind: 'developer', retirements: retire(owned) })).toEqual({
        ok: true,
        violations: []
      })
    })

    it('should still reject a removal that no record owns', async () => {
      const file = `${GEN}/components/places/PlaceCard.tsx`
      await rm(path.join(staged, file))

      expect(await kinds({ runKind: 'developer' })).toEqual([`removed:${file}`])
    })

    it('should allow a developer change to a frozen page', async () => {
      await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')

      expect(await run({ runKind: 'developer' })).toEqual({ ok: true, violations: [] })
    })

    it('should report a record whose output is still present as pending', async () => {
      const owned = await computeOwned(base, { pageId: PLACES })

      const found = (await run({ runKind: 'developer', retirements: retire(owned) })).violations

      expect(found.every((v) => v.kind === 'pending-retirement')).toBe(true)
      expect(found.map((v) => v.path)).toContain(`${GEN}/components/places/PlaceCard.tsx`)
      expect(found.map((v) => v.cause).join(' ')).toContain('still listed')
    })

    it('should flag a retired component by its own identity only', async () => {
      const owned = await computeOwned(base, { pageId: PLACES, componentId: 'p1' })

      const found = (await run({ runKind: 'developer', retirements: retire(owned, 'p1') }))
        .violations

      expect(found.map((v) => v.cause).join(' ')).toContain('PlaceCard')
      expect(found.map((v) => v.cause).join(' ')).not.toContain('Button')
    })
  })

  describe('when automation touches retirements', () => {
    it('should reject an edit to the retirements file', async () => {
      await put(staged, `${SNAP}/retirements.json`, {
        ...emptyRetirements(),
        retired: [{ pageId: BUTTONS, name: 'x', reason: 'y', owned: [], sharedExclusions: [] }]
      })

      expect(await kinds()).toContain(`retirement-edit:${SNAP}/retirements.json`)
    })
  })

  describe('when a shared input changes', () => {
    it('should reject tokens unless a foundation page was captured', async () => {
      await put(staged, `${GEN}/tokens/tokens.css`, 'tokens v2')

      expect(await kinds()).toEqual([`out-of-scope-change:${GEN}/tokens/tokens.css`])
      expect(await run({ foundationsCaptured: true })).toEqual({ ok: true, violations: [] })
    })
  })

  describe('when an artifact has no owner or cannot be parsed', () => {
    it('should reject an unowned generated file', async () => {
      await put(staged, `${GEN}/components/mystery/Thing.tsx`, 'x')

      expect(await kinds()).toEqual([`out-of-scope-change:${GEN}/components/mystery/Thing.tsx`])
    })

    it('should report an unreadable aggregate instead of passing it', async () => {
      await put(staged, `${REVIEW}/catalog.json`, '{ not json')

      expect((await run()).violations[0]?.kind).toBe('unreadable')
    })
  })

  describe('when a run tries to take over or fake ownership', () => {
    it('should reject a remap of a frozen component to a captured page', async () => {
      const moved = components.map((c) => (c.name === 'PlaceCard' ? { ...c, page: 'Buttons' } : c))
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

      const found = await kinds()

      expect(found).toContain(`out-of-scope-change:${GEN}/components/places/PlaceCard.tsx`)
      expect(
        (await run()).violations.find((violation) => violation.path.endsWith('PlaceCard.tsx'))
          ?.owner
      ).toEqual({ pageId: PLACES, componentId: 'p1' })
    })

    it('should preserve ID ownership when a manifest changes only a frozen page label', async () => {
      const moved = components.map((c) => (c.name === 'PlaceCard' ? { ...c, page: 'Buttons' } : c))
      await put(staged, `${GEN}/components/manifest.ts`, manifest(moved))

      expect((await run()).violations.map((v) => v.cause).join(' ')).toContain(
        'which was not captured'
      )
    })

    it('should reject an aggregate that did not exist before', async () => {
      await put(staged, `${SNAP}/generation-inventory.json`, {
        schema: 1,
        review: [],
        promotion: []
      })

      expect((await run()).violations[0]?.cause).toContain('created')
    })

    it('should reject duplicate entry keys instead of keeping the last one', async () => {
      await put(staged, `${GEN}/components/manifest.ts`, manifest([...components, components[0]!]))

      expect((await run()).violations.map((v) => v.kind)).toContain('unreadable')
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

      it('should keep rejecting a Review token change that affects an unselected page', async () => {
        await put(base, `${GEN}/tokens/tokens.css`, ':root { --color: red; }')
        await put(staged, `${GEN}/tokens/tokens.css`, ':root { --color: blue; }')
        for (const root of [base, staged])
          await put(
            root,
            `${GEN}/components/places/PlaceCard.module.css`,
            '.a { color: var(--color); }'
          )

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

    describe('and a partial stage writes library bytes', () => {
      it('should reject a staged library change', async () => {
        await put(staged, 'components/buttons/Button.tsx', 'new')

        expect(await kinds({ stage: 'partial', captured: [BUTTONS, PLACES] })).toEqual([
          'out-of-scope-change:components/buttons/Button.tsx'
        ])
      })

      it('should reject a staged library addition', async () => {
        await put(staged, 'components/buttons/Extra.tsx', 'new')

        expect(await kinds({ stage: 'partial', captured: [BUTTONS, PLACES] })).toEqual([
          'out-of-scope-change:components/buttons/Extra.tsx'
        ])
      })

      it('should reject a staged removal of library entries', async () => {
        await put(staged, 'components/promotion.json', {
          schema: 1,
          files: {},
          components: ['Button']
        })

        expect(await kinds({ stage: 'partial', captured: [BUTTONS, PLACES] })).toEqual([
          'out-of-scope-change:components/promotion.json'
        ])
      })
    })

    describe('and a complete stage mutates the library', () => {
      it('should reject a promotion entry the run does not own', async () => {
        await put(staged, 'components/promotion.json', {
          schema: 1,
          files: {},
          components: ['Button', 'PlaceCard', 'Unowned']
        })

        expect(await kinds({ stage: 'complete', ...cliDefaults })).toEqual([
          'aggregate-entry-changed:components/promotion.json'
        ])
      })

      it('should reject a removed library entry point', async () => {
        await rm(path.join(staged, 'index.ts'))

        expect(await kinds({ stage: 'complete', ...cliDefaults })).toEqual(
          expect.arrayContaining(['removed:index.ts'])
        )
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

    it('should allow a promotion to add and change library files', async () => {
      await put(staged, 'components/buttons/Button.tsx', 'button2')
      await put(staged, 'components/b.ts', 'b')

      expect(await run(watch)).toEqual({ ok: true, violations: [] })
    })

    it('should preserve existing unowned library files even in the narrow promotion scope', async () => {
      await put(staged, 'components/a.ts', 'changed')
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

    it('should reject deleting a hand-written library directory', async () => {
      await rm(path.join(staged, 'components/Modal'), { recursive: true })

      expect(await kinds(watch)).toEqual(['removed:components/Modal/Modal.tsx'])
    })
  })

  describe('when a retirement touches the promoted library', () => {
    const retirePlace = async (): Promise<Retirements> => ({
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
    })

    it('should own the library copy, the exports and the promotion entry', async () => {
      const owned = (await retirePlace()).retired[0]!.owned

      expect(owned).toEqual(
        expect.arrayContaining([
          'components/places/PlaceCard.tsx',
          'components/places/PlaceCard.figma.ts',
          'index.ts#value:PlaceCard',
          'index.ts#type:PlaceCardProps',
          'components/promotion.json#component:PlaceCard'
        ])
      )
      expect(owned.join(' ')).not.toContain('Button')
      expect(owned.join(' ')).not.toContain('Modal')
    })

    it('should flag a retired component the library still exports or lists', async () => {
      const found = (await run({ runKind: 'developer', retirements: await retirePlace() }))
        .violations

      expect(found.map((v) => v.cause).join(' ')).toContain('value:PlaceCard')
      expect(found.map((v) => v.cause).join(' ')).toContain('component:PlaceCard')
    })

    it('should require what the retirement shares with live items to stay', async () => {
      const retirements = await retirePlace()
      retirements.retired[0]!.sharedExclusions = [`${GEN}/components/buttons/Button.tsx`]
      await rm(path.join(staged, `${GEN}/components/buttons/Button.tsx`))

      expect(
        (await run({ runKind: 'developer', retirements })).violations.map((v) => v.cause)
      ).toContain('shared with live items by the retirement of Places, but missing')
    })
  })
})
