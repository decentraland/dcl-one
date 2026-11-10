import { mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { checkScope } from './check-scope.js'
import { stageOutput, type StagedOutput } from './stage.js'
import { emptyRetirements } from './retirements.js'
import {
  BUTTONS,
  PLACES,
  GEN,
  SNAP,
  components,
  manifest,
  put,
  writeTree
} from './test-fixtures.js'

describe('when only Review output is staged', () => {
  let base: string
  let staged: StagedOutput
  beforeEach(async () => {
    base = await mkdtemp(path.join(os.tmpdir(), 'partial-output-stage-'))
    await writeTree(base)
    await put(base, 'components/tokens/tokens.css', ':root { --shared: red; }')
    await put(base, 'components/places/PlaceCard.module.css', '.card { color: var(--shared); }')
    staged = await stageOutput(base, GEN, [])
  })
  afterEach(async () => {
    await staged.cleanup()
    await rm(base, { recursive: true, force: true })
  })
  it('should preserve the unstaged library without treating it as a changed shared input', async () => {
    expect(
      await checkScope({
        baseDir: base,
        stagedDir: staged.stagedDir,
        capturedPageIds: [],
        runKind: 'automated',
        stage: staged.completeness,
        retirements: emptyRetirements(),
        directories: [GEN],
        files: []
      })
    ).toEqual({ ok: true, violations: [] })
  })
  it('should retain stable ownership for a newly allocated name on the selected page', async () => {
    await staged.cleanup()
    await put(base, `${SNAP}/components/Buttons.json`, {
      v: 1,
      page: { id: BUTTONS, name: 'Buttons' },
      components: [
        { id: 'b1', name: 'Button', base: { id: '10:1' }, others: [] },
        { id: 'b2', name: 'Button', others: [] }
      ]
    })
    staged = await stageOutput(base, GEN, [
      { path: 'components/buttons/Button2.tsx', content: 'selected duplicate' },
      {
        path: 'components/manifest.ts',
        content: manifest([
          ...components,
          { name: 'Button2', page: 'Buttons', id: 'b2', file: 'buttons/Button2', variants: [] }
        ])
      }
    ])
    expect(
      await checkScope({
        baseDir: base,
        stagedDir: staged.stagedDir,
        capturedPageIds: [BUTTONS],
        runKind: 'automated',
        stage: staged.completeness,
        retirements: emptyRetirements(),
        directories: [GEN],
        files: []
      })
    ).toEqual({ ok: true, violations: [] })
  })
  it('should check shared exclusions in the unchanged base when the library is not staged', async () => {
    await put(base, 'components/assets/shared.svg', '<svg/>')
    expect(
      (
        await checkScope({
          baseDir: base,
          stagedDir: staged.stagedDir,
          capturedPageIds: [],
          runKind: 'automated',
          stage: staged.completeness,
          retirements: {
            schema: 1,
            retired: [
              {
                pageId: 'gone',
                name: 'Gone',
                reason: 'Retired',
                owned: [],
                sharedExclusions: ['components/assets/shared.svg']
              }
            ]
          },
          directories: [GEN],
          files: []
        })
      ).ok
    ).toBe(true)
  })
  describe('when shared icons change behind the FigmaView runtime', () => {
    beforeEach(async () => {
      for (const directory of [base, staged.stagedDir])
        await put(
          directory,
          `${GEN}/components/places/PlaceCard.tsx`,
          'import { FigmaView } from "../../../components/figma-module-runtime"'
        )
      await put(base, `${GEN}/icons/Icons.tsx`, 'old registry')
      await put(staged.stagedDir, `${GEN}/icons/Icons.tsx`, 'changed registry')
    })
    it('should reject the dependency change for an unselected page', async () => {
      const result = await checkScope({
        baseDir: base,
        stagedDir: staged.stagedDir,
        capturedPageIds: [BUTTONS],
        foundationsCaptured: true,
        runKind: 'automated',
        stage: staged.completeness,
        retirements: emptyRetirements(),
        directories: [GEN],
        files: []
      })
      expect(result.violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: `${GEN}/components/places/PlaceCard.tsx`,
            kind: 'out-of-scope-change',
            cause: expect.stringContaining('shared input')
          })
        ])
      )
    })
    it('should allow the change when every dependent page was captured', async () => {
      expect(
        (
          await checkScope({
            baseDir: base,
            stagedDir: staged.stagedDir,
            capturedPageIds: [BUTTONS, PLACES],
            foundationsCaptured: true,
            runKind: 'automated',
            stage: staged.completeness,
            retirements: emptyRetirements(),
            directories: [GEN],
            files: []
          })
        ).ok
      ).toBe(true)
    })
  })
})
