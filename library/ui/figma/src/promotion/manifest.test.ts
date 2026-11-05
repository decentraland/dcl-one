import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import type { App } from '../app.js'
import { readMaterialized } from '../lfs.js'
import { loadSpecsFromSource } from '../specs.js'
import {
  createPromotionManifest,
  runtimeFiles,
  verifyPromotionManifest,
  writePromotionManifest
} from './manifest.js'

vi.mock('../specs.js', () => ({ loadSpecsFromSource: vi.fn() }))

describe('when binding promotion to reviewed source', () => {
  let root: string
  let app: App
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'promotion-test-'))
    app = {
      cwd: path.join(root, 'figma'),
      config: { selection: { exceptions: [{ componentId: 'unready' }] } },
      createSource: () => ({})
    } as unknown as App
    vi.mocked(loadSpecsFromSource).mockResolvedValue({
      specs: [],
      skippedPages: [],
      nameCollisions: []
    })
    for (const file of [
      'figma/review/src/generated/components/Test.tsx',
      'figma/figma-snapshot/components/Page.json',
      ...runtimeFiles.map((name) => `figma/review/src/components/${name}`),
      'figma/review/src/review/catalog.json',
      'figma/review/src/review/references.json',
      'figma/review/public/dcl-ui-review/fidelity.json',
      'figma/review/public/dcl-ui-review/references/source.png'
    ]) {
      await mkdir(path.dirname(path.join(root, file)), { recursive: true })
      await writeFile(path.join(root, file), '{}')
    }
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should exclude interrupted atomic writes from the reviewed input seal', async () => {
    await writePromotionManifest(app)
    for (const directory of [
      'figma/review/src/generated',
      'figma/review/public/dcl-ui-review',
      'figma/figma-snapshot'
    ])
      await writeFile(
        path.join(root, directory, '.figma-tmp-00000000-0000-4000-8000-000000000000'),
        'partial'
      )
    expect((await verifyPromotionManifest(app)).schema).toBe(1)
  })
  it('should use the committed promotion scope without changing Review exceptions', async () => {
    await createPromotionManifest(app)
    expect(loadSpecsFromSource).toHaveBeenCalledWith({}, app.config, 'promotion')
  })
  it('should accept the exact source reviewed by the manifest', async () => {
    await writePromotionManifest(app)
    expect((await verifyPromotionManifest(app)).schema).toBe(1)
  })
  it('should write no file key into the manifest even when one is configured', async () => {
    vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
    try {
      const { file } = await writePromotionManifest(app)
      const written = await readFile(file, 'utf8')
      expect(written).not.toContain('FAKEKEY000example000000')
      expect(JSON.parse(written)).not.toHaveProperty('fileKey')
    } finally {
      vi.unstubAllEnvs()
    }
  })
  it('should fail loudly when an input is a Git LFS pointer', async () => {
    await writeFile(
      path.join(root, 'figma/review/public/dcl-ui-review/references/source.png'),
      'version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n'
    )
    await expect(createPromotionManifest(app)).rejects.toThrow('Git LFS pointer')
  })
  it('should fail loudly when the sealed manifest itself is a Git LFS pointer', async () => {
    await writeFile(
      path.join(root, 'figma/review/src/review/promotion.json'),
      'version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n'
    )
    await expect(verifyPromotionManifest(app)).rejects.toThrow('Git LFS pointer')
  })
  it.each([
    'figma/review/src/generated/components/Test.tsx',
    'figma/review/src/components/figma-module-runtime.tsx',
    'figma/review/public/dcl-ui-review/references/source.png',
    'figma/figma-snapshot/components/Page.json'
  ])('should reject changed source or evidence at %s', async (file) => {
    await writePromotionManifest(app)
    await writeFile(path.join(root, file), 'changed')
    await expect(verifyPromotionManifest(app)).rejects.toThrow('Reviewed source changed')
  })

  describe('and the specs are sealed by hash (schema 2)', () => {
    const manifestFile = () => path.join(root, 'figma/review/src/review/promotion.json')
    const card = {
      name: 'Card',
      page: 'Places',
      needsManual: [],
      props: [{ name: 'state', options: ['default', 'hover'] }],
      semantics: { role: 'action' },
      variants: [{ key: 'state:default' }, { key: 'state:hover' }],
      css: 'x'.repeat(100_000)
    }
    const specs = (...list: object[]) =>
      vi.mocked(loadSpecsFromSource).mockResolvedValue({
        specs: list as never,
        skippedPages: [],
        nameCollisions: []
      })
    const sealed = async () => JSON.parse(await readFile(manifestFile(), 'utf8'))
    const writeSealed = (value: unknown) => writeFile(manifestFile(), JSON.stringify(value))

    beforeEach(() => specs(card))

    it('should write one name and hash per spec instead of the spec itself', async () => {
      await writePromotionManifest(app)
      const written = await readFile(manifestFile(), 'utf8')

      expect(written.length).toBeLessThan(10_000)
      expect(JSON.parse(written)).toMatchObject({
        schema: 2,
        components: [{ name: 'Card', hash: expect.stringMatching(/^[0-9a-f]{64}$/) }]
      })
    })

    it('should verify a fresh seal and hand promotion the rebuilt specs', async () => {
      await writePromotionManifest(app)

      expect((await verifyPromotionManifest(app)).components).toEqual([card])
    })

    // The component name and component count stay the same; only the spec behind the name changes,
    // so these are caught by spec hashing alone (every sealed file is unchanged).
    it.each([
      ['a variant added', { variants: [...card.variants, { key: 'state:pressed' }] }],
      ['a variant removed', { variants: card.variants.slice(0, 1) }],
      ['a variant renamed', { variants: [card.variants[0], { key: 'state:focused' }] }],
      ['its props changed', { props: [{ name: 'state', options: ['default'] }] }],
      ['its semantics changed', { semantics: { role: 'navigation' } }]
    ])('should reject a spec with %s and name the component', async (_case, change) => {
      await writePromotionManifest(app)
      specs({ ...card, ...change })

      await expect(verifyPromotionManifest(app)).rejects.toThrow(
        'Stale: component Card no longer matches its sealed spec hash.'
      )
    })

    it('should reject a changed needs-manual reason and name the component', async () => {
      const blocked = { ...card, name: 'Blocked', needsManual: ['slot-property: one'] }
      specs(card, blocked)
      await writePromotionManifest(app)
      specs(card, { ...blocked, needsManual: ['slot-property: two'] })

      await expect(verifyPromotionManifest(app)).rejects.toThrow(
        'Stale: excluded component Blocked changed its needs-manual reasons.'
      )
    })

    it('should still reject a changed sealed file when every spec is unchanged', async () => {
      await writePromotionManifest(app)
      await writeFile(path.join(root, 'figma/figma-snapshot/components/Page.json'), 'changed')

      await expect(verifyPromotionManifest(app)).rejects.toThrow(
        'Stale: figma/figma-snapshot/components/Page.json changed since review.'
      )
    })

    it.each([
      [
        'an unknown schema version',
        (m: Record<string, unknown>) => ({ ...m, schema: 3 }),
        'field schema is 3; expected 2'
      ],
      [
        'a missing component hash',
        (m: Record<string, unknown>) => ({ ...m, components: [{ name: 'Card' }] }),
        'field components[0].hash (Card) is not a sha256 hex digest'
      ],
      [
        'a short component hash',
        (m: Record<string, unknown>) => ({ ...m, components: [{ name: 'Card', hash: 'abc' }] }),
        'field components[0].hash (Card) is not a sha256 hex digest'
      ],
      [
        'a component without a name',
        (m: Record<string, unknown>) => ({ ...m, components: [{ hash: '0'.repeat(64) }] }),
        'field components[0].name is missing'
      ],
      [
        'a malformed file digest',
        (m: Record<string, unknown>) => ({ ...m, files: { 'figma/x.json': 'nope' } }),
        'field files["figma/x.json"] is not a sha256 hex digest'
      ],
      [
        'components that are not a list',
        (m: Record<string, unknown>) => ({ ...m, components: {} }),
        'field components is missing or not an array'
      ],
      [
        'missing excluded components',
        (m: Record<string, unknown>) => ({ ...m, excluded: undefined }),
        'field excluded is missing or not an array'
      ],
      [
        'a null excluded entry',
        (m: Record<string, unknown>) => ({ ...m, excluded: [null] }),
        'field excluded[0] is not an object'
      ],
      [
        'a primitive excluded entry',
        (m: Record<string, unknown>) => ({ ...m, excluded: ['Blocked'] }),
        'field excluded[0] is not an object'
      ],
      [
        'an excluded entry without a name',
        (m: Record<string, unknown>) => ({ ...m, excluded: [{ reasons: [] }] }),
        'field excluded[0].name is missing'
      ],
      [
        'an excluded entry without reasons',
        (m: Record<string, unknown>) => ({ ...m, excluded: [{ name: 'Blocked' }] }),
        'field excluded[0].reasons (Blocked) is not a list of strings'
      ],
      [
        'an excluded entry with non-string reasons',
        (m: Record<string, unknown>) => ({ ...m, excluded: [{ name: 'Blocked', reasons: [1] }] }),
        'field excluded[0].reasons (Blocked) is not a list of strings'
      ],
      [
        'a null component entry',
        (m: Record<string, unknown>) => ({ ...m, components: [null] }),
        'field components[0] is not an object'
      ]
    ])('should reject %s with a diagnostic naming the field', async (_case, corrupt, message) => {
      await writePromotionManifest(app)
      await writeSealed(corrupt(await sealed()))

      await expect(verifyPromotionManifest(app)).rejects.toThrow(message)
    })

    it('should reject a manifest that is not JSON', async () => {
      await writeFile(manifestFile(), '{ not json')

      await expect(verifyPromotionManifest(app)).rejects.toThrow(
        'Promotion manifest is not valid JSON.'
      )
    })

    it('should keep the committed seal on schema 2, with no file key', async () => {
      const committed = JSON.parse(
        (
          await readMaterialized(
            path.resolve(import.meta.dirname, '../../review/src/review/promotion.json')
          )
        ).toString('utf8')
      )

      expect(committed.schema).toBe(2)
      expect(committed).not.toHaveProperty('fileKey')
    })

    it('should report a legacy manifest (schema 1) as legacy and stale, never as verified', async () => {
      await writeSealed({ schema: 1, files: {}, components: [], excluded: [] })

      await expect(verifyPromotionManifest(app)).rejects.toThrow(
        'The sealed promotion manifest uses the legacy schema 1 (embedded specs)'
      )
    })
  })
})
