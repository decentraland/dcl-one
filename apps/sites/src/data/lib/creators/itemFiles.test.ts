import { describe, expect, it } from 'vitest'
import JSZip from 'jszip'
import { manifest } from '@dcl-regenesislabs/wearable-validator'
import { BodyShapeType } from './items'
import {
  ItemFileError,
  MAX_EMOTE_FILE_SIZE,
  MAX_SKIN_FILE_SIZE,
  MAX_THUMBNAIL_FILE_SIZE,
  MAX_WEARABLE_FILE_SIZE,
  cleanAssetName,
  cleanContentModelKeys,
  findOrphanedAuxiliaryFiles,
  getBodyShapeTypeFromContents,
  getExtension,
  getRequiredCounterpartFile,
  hasFacialExpressions,
  isModelPath,
  loadItemFile,
  stripWrappingFolder,
  toMB,
  validateVideoFile,
  createZipInflater,
  MAX_VIDEO_FILE_SIZE,
  MAX_ZIP_FILE_SIZE
} from './itemFiles'

const blob = (size = 10) => new Blob([new Uint8Array(size)])

async function zipFile(entries: Record<string, string | Uint8Array | Blob>, name = 'item.zip'): Promise<File> {
  const zip = new JSZip()
  for (const [path, content] of Object.entries(entries)) {
    zip.file(path, content)
  }
  const buffer = await zip.generateAsync({ type: 'arraybuffer' })
  return new File([buffer], name)
}

async function expectItemFileError(promise: Promise<unknown>, messageKey: string) {
  try {
    await promise
    expect.fail('expected an ItemFileError')
  } catch (error) {
    expect(error).toBeInstanceOf(ItemFileError)
    expect((error as ItemFileError).messageKey).toBe(messageKey)
  }
}

describe('getExtension / cleanAssetName / toMB', () => {
  it('extracts the lowercased extension and cleans asset names', () => {
    expect(getExtension('Model.GLB')).toBe('.glb')
    expect(getExtension('no-extension')).toBeNull()
    expect(cleanAssetName('my_cool-hat.v2.glb')).toBe('my cool hat v2')
    expect(toMB(3 * 1024 * 1024)).toBe(3)
  })
})

describe('isModelPath', () => {
  it('accepts models and base textures but not thumbnails or auxiliary PNGs', () => {
    expect(isModelPath('model.glb')).toBe(true)
    expect(isModelPath('eyes.png')).toBe(true)
    expect(isModelPath('thumbnail.png')).toBe(false)
    expect(isModelPath('eyes_mask.png')).toBe(false)
    expect(isModelPath('eyes_expressions.png')).toBe(false)
  })
})

describe('auxiliary file pairing', () => {
  it('maps each auxiliary file to its required counterpart', () => {
    expect(getRequiredCounterpartFile('eyes_mask.png')).toBe('eyes.png')
    expect(getRequiredCounterpartFile('eyes_expressions.png')).toBe('eyes.png')
    expect(getRequiredCounterpartFile('eyes_expressions_mask.png')).toBe('eyes_expressions.png')
    expect(getRequiredCounterpartFile('eyes.png')).toBeNull()
  })

  it('finds orphans case-insensitively', () => {
    expect(findOrphanedAuxiliaryFiles({ 'Eyes.PNG': 1, 'eyes_mask.png': 1 })).toEqual([])
    expect(findOrphanedAuxiliaryFiles({ 'eyes_mask.png': 1 })).toEqual([
      { orphan: 'eyes_mask.png', expected: 'eyes.png' }
    ])
  })
})

describe('stripWrappingFolder', () => {
  it('unwraps a single wrapper folder recursively', () => {
    const result = stripWrappingFolder({ 'outer/inner/model.glb': blob(), 'outer/inner/tex.png': blob() })
    expect(Object.keys(result).sort()).toEqual(['model.glb', 'tex.png'])
  })

  it('never unwraps body-shape folders or mixed roots', () => {
    const bodyShaped = { 'male/model.glb': blob() }
    expect(stripWrappingFolder(bodyShaped)).toEqual(bodyShaped)
    const mixed = { 'a/x.glb': blob(), 'b/y.glb': blob() }
    expect(stripWrappingFolder(mixed)).toEqual(mixed)
  })
})

describe('getBodyShapeTypeFromContents / cleanContentModelKeys', () => {
  it('detects the body shape from folder prefixes', () => {
    expect(getBodyShapeTypeFromContents({ 'male/m.glb': 1, 'female/f.glb': 1 })).toBe(BodyShapeType.BOTH)
    expect(getBodyShapeTypeFromContents({ 'male/m.glb': 1 })).toBe(BodyShapeType.MALE)
    expect(getBodyShapeTypeFromContents({ 'm.glb': 1 })).toBeNull()
  })

  it('flattens model keys for a single shape and keeps them prefixed for both', () => {
    const contents = { 'male/model.glb': blob(), 'male/sound.mp3': blob() }
    expect(Object.keys(cleanContentModelKeys(contents)).sort()).toEqual(['male/sound.mp3', 'model.glb'])
    expect(Object.keys(cleanContentModelKeys(contents, BodyShapeType.BOTH)).sort()).toEqual([
      'male/model.glb',
      'male/sound.mp3'
    ])
  })
})

describe('loadItemFile', () => {
  it('rejects unsupported extensions', async () => {
    await expectItemFileError(loadItemFile(new File([blob()], 'file.txt')), 'wrong_extension')
  })

  it('wraps a bare model file as single-content with no fixed body shape', async () => {
    const result = await loadItemFile(new File([blob()], 'hat.glb'))
    expect(result.model).toBe('hat.glb')
    expect(result.bodyShape).toBeNull()
    expect(Object.keys(result.contents)).toEqual(['hat.glb'])
  })

  it('treats a bare PNG as an image wearable for both shapes', async () => {
    const result = await loadItemFile(new File([blob()], 'eyes.png'))
    expect(result.bodyShape).toBe(BodyShapeType.BOTH)
  })

  it('rejects files over the size cap', async () => {
    const big = new File([new Uint8Array(9 * 1024 * 1024 + 1)], 'huge.glb')
    await expectItemFileError(loadItemFile(big), 'file_too_big')
  })

  it('rejects a zip with too many entries', async () => {
    const entries: Record<string, string> = { 'model.glb': 'glb-bytes' }
    for (let index = 0; index < 500; index++) entries[`extra-${index}.png`] = 'x'
    await expectItemFileError(loadItemFile(await zipFile(entries)), 'too_many_files')
  })

  it('refuses a zip bigger than anything it may unpack to, without opening it', async () => {
    const file = await zipFile({ 'model.glb': blob() })
    Object.defineProperty(file, 'size', { value: MAX_ZIP_FILE_SIZE + 1 })
    await expectItemFileError(loadItemFile(file), 'zip_too_big')
  })

  it('stops inflating entries as soon as the unpacked total passes the budget', async () => {
    const zip = new JSZip()
    zip.file('a.bin', new Uint8Array(600))
    zip.file('b.bin', new Uint8Array(600))
    const loaded = await JSZip.loadAsync(await zip.generateAsync({ type: 'arraybuffer' }))
    const inflater = createZipInflater(1000)
    await expect(inflater.inflate(loaded.file('a.bin')!)).resolves.toHaveProperty('size', 600)
    await expectItemFileError(inflater.inflate(loaded.file('b.bin')!), 'zip_too_big')
  })

  it('unzips a flat zip and finds the main model', async () => {
    const file = await zipFile({ 'model.glb': 'glb-bytes', 'texture.png': 'texture' })
    const result = await loadItemFile(file)
    expect(result.model).toBe('model.glb')
    expect(Object.keys(result.contents).sort()).toEqual(['model.glb', 'texture.png'])
  })

  it('takes back a downloaded item whose body shapes were folded into the root, with no body shape set', async () => {
    const result = await loadItemFile(
      await zipFile({ 'hat.glb': 'glb', 'hat.png': 'texture', 'thumbnail.png': 'thumb', 'image.png': 'catalyst' })
    )
    expect(result.model).toBe('hat.glb')
    expect(result.bodyShape).toBeNull()
    expect(Object.keys(result.contents).sort()).toEqual(['hat.glb', 'hat.png', 'image.png', 'thumbnail.png'])
  })

  it('strips a wrapping folder inside a zip', async () => {
    const file = await zipFile({ 'MyItem/model.glb': 'glb-bytes' })
    const result = await loadItemFile(file)
    expect(result.model).toBe('model.glb')
  })

  it('detects body-shape folders and flattens single-shape zips', async () => {
    const file = await zipFile({ 'female/model.glb': 'glb-bytes' })
    const result = await loadItemFile(file)
    expect(result.bodyShape).toBe(BodyShapeType.FEMALE)
    expect(result.model).toBe('model.glb')
    expect(Object.keys(result.contents)).toEqual(['model.glb'])
  })

  it('keeps both-shape zips prefixed', async () => {
    const file = await zipFile({ 'male/model.glb': 'm', 'female/model.glb': 'f' })
    const result = await loadItemFile(file)
    expect(result.bodyShape).toBe(BodyShapeType.BOTH)
    expect(Object.keys(result.contents).sort()).toEqual(['female/model.glb', 'male/model.glb'])
  })

  it('reads a wearable.json manifest and prefers its data', async () => {
    const manifest = {
      name: 'Cool Hat',
      rarity: 'legendary',
      utility: 'Glows at night',
      data: {
        category: 'hat',
        replaces: [],
        hides: [],
        tags: ['cool'],
        representations: [
          {
            bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
            mainFile: 'model.glb',
            contents: ['model.glb']
          }
        ]
      }
    }
    const file = await zipFile({ 'wearable.json': JSON.stringify(manifest), 'model.glb': 'glb-bytes' })
    const result = await loadItemFile(file)
    expect(result.wearable?.name).toBe('Cool Hat')
    expect(result.wearable?.utility).toBe('Glows at night')
    expect(result.bodyShape).toBe(BodyShapeType.MALE)
    expect(result.model).toBe('model.glb')
    expect(result.contents['wearable.json']).toBeUndefined()
  })

  it('rejects a manifest that references missing files', async () => {
    const manifest = {
      name: 'Cool Hat',
      data: {
        category: 'hat',
        replaces: [],
        hides: [],
        tags: [],
        representations: [
          {
            bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
            mainFile: 'missing.glb',
            contents: ['missing.glb']
          }
        ]
      }
    }
    const file = await zipFile({ 'wearable.json': JSON.stringify(manifest), 'model.glb': 'glb-bytes' })
    await expectItemFileError(loadItemFile(file), 'manifest_file_missing')
  })

  it('reads an emote.json manifest', async () => {
    const file = await zipFile({
      'emote.json': JSON.stringify({ name: 'Wave', play_mode: 'loop', utility: 'Unlocks a dance floor' }),
      'emote.glb': 'glb'
    })
    const result = await loadItemFile(file)
    expect(result.emote?.play_mode).toBe('loop')
    expect(result.emote?.utility).toBe('Unlocks a dance floor')
    expect(result.bodyShape).toBe(BodyShapeType.BOTH)
  })

  it('rejects an emote.json manifest with wrongly typed fields', async () => {
    const file = await zipFile({ 'emote.json': JSON.stringify({ name: 42 }), 'emote.glb': 'glb' })
    await expectItemFileError(loadItemFile(file), 'invalid_manifest')
  })

  it('validates the wearable.json schema like the legacy builder-client', async () => {
    const manifest = (data: Record<string, unknown>) =>
      JSON.stringify({
        name: 'Hat',
        data: {
          category: 'hat',
          replaces: [],
          hides: [],
          tags: [],
          representations: [
            {
              bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
              mainFile: 'model.glb',
              contents: ['model.glb']
            }
          ],
          ...data
        }
      })
    await expectItemFileError(
      loadItemFile(await zipFile({ 'wearable.json': manifest({ category: 'spaceship' }), 'model.glb': 'glb' })),
      'invalid_manifest'
    )
    await expectItemFileError(
      loadItemFile(await zipFile({ 'wearable.json': manifest({ tags: undefined }), 'model.glb': 'glb' })),
      'invalid_manifest'
    )
    await expectItemFileError(
      loadItemFile(
        await zipFile({
          'wearable.json': manifest({
            representations: [{ bodyShapes: ['urn:something:else'], mainFile: 'model.glb', contents: ['model.glb'] }]
          }),
          'model.glb': 'glb'
        })
      ),
      'invalid_manifest'
    )
    const result = await loadItemFile(await zipFile({ 'wearable.json': manifest({}), 'model.glb': 'glb' }))
    expect(result.bodyShape).toBe(BodyShapeType.MALE)
  })

  it('rejects an orphaned mask texture even when a wearable.json names the main file', async () => {
    const manifest = JSON.stringify({
      name: 'Eyes',
      data: {
        category: 'eyes',
        replaces: [],
        hides: [],
        tags: [],
        representations: [
          {
            bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
            mainFile: 'eyes.png',
            contents: ['eyes.png']
          }
        ]
      }
    })
    await expectItemFileError(
      loadItemFile(await zipFile({ 'wearable.json': manifest, 'eyes.png': 'png', 'other_mask.png': 'png' })),
      'orphaned_auxiliary_file'
    )
  })

  describe('smart wearables', () => {
    const wearableManifest = JSON.stringify({
      name: 'Glasses',
      data: {
        category: 'eyewear',
        replaces: [],
        hides: [],
        tags: [],
        representations: [
          {
            bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
            mainFile: 'glasses.glb',
            contents: ['glasses.glb']
          }
        ]
      }
    })
    const scene = (overrides: Record<string, unknown> = {}) =>
      JSON.stringify({
        main: 'bin/game.js',
        scene: { parcels: ['0,0'], base: '0,0' },
        requiredPermissions: ['USE_FETCH', 'OPEN_EXTERNAL_LINK'],
        ...overrides
      })
    const files = { 'wearable.json': wearableManifest, 'glasses.glb': 'glb', 'bin/game.js': 'code' }

    it('loads the scene code, keeps a normalized scene.json and forces both body shapes', async () => {
      const result = await loadItemFile(await zipFile({ ...files, 'scene.json': scene() }))
      expect(result.scene?.main).toBe('bin/game.js')
      expect(result.scene?.requiredPermissions).toEqual(['USE_FETCH', 'OPEN_EXTERNAL_LINK'])
      expect(Object.keys(result.contents).sort()).toEqual(['bin/game.js', 'glasses.glb', 'scene.json'])
      expect(JSON.parse(await result.contents['scene.json'].text())).toMatchObject({ main: 'bin/game.js' })
      expect(result.bodyShape).toBe(BodyShapeType.BOTH)
    })

    it('accepts a scene.json without wearable.json, preferring the 3D model over loose PNGs', async () => {
      const result = await loadItemFile(
        await zipFile({
          'scene.json': scene(),
          'menupic.png': 'png',
          'model.glb': 'glb',
          'bin/game.js': 'code',
          'asset.json': '{}'
        })
      )
      expect(result.wearable).toBeUndefined()
      expect(result.scene?.requiredPermissions).toEqual(['USE_FETCH', 'OPEN_EXTERNAL_LINK'])
      expect(result.model).toBe('model.glb')
      expect(result.bodyShape).toBe(BodyShapeType.BOTH)
      expect(Object.keys(result.contents).sort()).toEqual([
        'asset.json',
        'bin/game.js',
        'menupic.png',
        'model.glb',
        'scene.json'
      ])
    })

    it('flattens a single body-shape folder of a scene zip: the model serves both shapes', async () => {
      const result = await loadItemFile(
        await zipFile({ 'scene.json': scene(), 'female/model.glb': 'glb', 'bin/game.js': 'code' })
      )
      expect(result.model).toBe('model.glb')
      expect(result.bodyShape).toBe(BodyShapeType.BOTH)
      expect(Object.keys(result.contents).sort()).toEqual(['bin/game.js', 'model.glb', 'scene.json'])
    })

    it('takes the 3D model of a downloaded item zip, not its catalyst image or thumbnail', async () => {
      const result = await loadItemFile(
        await zipFile({ 'image.png': 'catalyst', 'thumbnail.png': 'thumb', 'male/hat.glb': 'glb' })
      )
      expect(result.model).toBe('hat.glb')
      expect(result.bodyShape).toBe(BodyShapeType.MALE)
      expect(Object.keys(result.contents).sort()).toEqual(['hat.glb', 'image.png', 'thumbnail.png'])
    })

    it('keeps a scene zip own folders: only a body-shape folder is stripped', async () => {
      const result = await loadItemFile(
        await zipFile({
          'scene.json': scene(),
          'models/hat.glb': 'glb',
          'models/hat.bin': 'bin',
          'images/a/tex.png': 'a',
          'images/b/tex.png': 'b',
          'bin/game.js': 'code'
        })
      )
      expect(result.model).toBe('models/hat.glb')
      expect(Object.keys(result.contents).sort()).toEqual([
        'bin/game.js',
        'images/a/tex.png',
        'images/b/tex.png',
        'models/hat.bin',
        'models/hat.glb',
        'scene.json'
      ])
      const nested = await loadItemFile(
        await zipFile({
          'scene.json': scene(),
          'female/models/hat.glb': 'glb',
          'female/models/hat.bin': 'bin',
          'bin/game.js': 'code'
        })
      )
      expect(nested.model).toBe('models/hat.glb')
      expect(Object.keys(nested.contents).sort()).toEqual([
        'bin/game.js',
        'models/hat.bin',
        'models/hat.glb',
        'scene.json'
      ])
    })

    it('still takes a texture called image.png inside a body-shape folder', async () => {
      const result = await loadItemFile(await zipFile({ 'male/image.png': 'png' }))
      expect(result.model).toBe('image.png')
    })

    it('rejects a nested scene.json next to a root wearable.json', async () => {
      await expectItemFileError(
        loadItemFile(
          await zipFile({
            'wearable.json': wearableManifest,
            'glasses.glb': 'glb',
            'project/scene.json': scene(),
            'project/bin/game.js': 'code'
          })
        ),
        'scene_manifest_nested'
      )
    })

    it('rejects per-body-shape scene.json copies that are not inside the same folder', async () => {
      await expectItemFileError(
        loadItemFile(
          await zipFile({
            'a/male/scene.json': scene(),
            'a/male/bin/game.js': 'code',
            'a/male/model.glb': 'glb',
            'b/female/scene.json': scene(),
            'b/female/bin/game.js': 'code',
            'b/female/model.glb': 'glb'
          })
        ),
        'scene_manifest_missing'
      )
    })

    it('finds the scene.json of a zipped project folder', async () => {
      const result = await loadItemFile(
        await zipFile({ 'my-sw/scene.json': scene(), 'my-sw/model.glb': 'glb', 'my-sw/bin/game.js': 'code' })
      )
      expect(result.scene?.main).toBe('bin/game.js')
      expect(Object.keys(result.contents).sort()).toEqual(['bin/game.js', 'model.glb', 'scene.json'])
    })

    it('round-trips a downloaded smart wearable: scene and code duplicated under male/ and female/', async () => {
      const result = await loadItemFile(
        await zipFile({
          'anime/thumbnail.png': 'thumb',
          'anime/image.png': 'catalyst',
          'anime/male/scene.json': scene(),
          'anime/male/bin/game.js': 'code',
          'anime/male/long.glb': 'glb-m',
          'anime/female/scene.json': JSON.stringify(JSON.parse(scene()), null, 2),
          'anime/female/bin/game.js': 'code',
          'anime/female/short.glb': 'glb-f'
        })
      )
      expect(result.scene?.main).toBe('bin/game.js')
      expect(result.bodyShape).toBe(BodyShapeType.BOTH)
      expect(Object.keys(result.contents).sort()).toEqual([
        'female/bin/game.js',
        'female/short.glb',
        'image.png',
        'male/bin/game.js',
        'male/long.glb',
        'scene.json',
        'thumbnail.png'
      ])
    })

    it('round-trips a downloaded smart wearable whose body shapes were folded into the root', async () => {
      const result = await loadItemFile(
        await zipFile({
          'thumbnail.png': 'thumb',
          'image.png': 'catalyst',
          'video.mp4': 'video',
          'scene.json': scene(),
          'bin/game.js': 'code',
          'glasses.glb': 'glb'
        })
      )
      expect(result.scene?.main).toBe('bin/game.js')
      expect(result.model).toBe('glasses.glb')
      expect(result.bodyShape).toBe(BodyShapeType.BOTH)
      expect(Object.keys(result.contents).sort()).toEqual([
        'bin/game.js',
        'glasses.glb',
        'image.png',
        'scene.json',
        'thumbnail.png',
        'video.mp4'
      ])
    })

    it('rejects a downloaded smart wearable whose male and female code differ', async () => {
      await expectItemFileError(
        loadItemFile(
          await zipFile({
            'male/scene.json': scene(),
            'male/bin/game.js': 'code A',
            'male/model.glb': 'glb',
            'female/scene.json': scene(),
            'female/bin/game.js': 'code B',
            'female/model.glb': 'glb'
          })
        ),
        'scene_code_mismatch'
      )
    })

    it('rejects a downloaded smart wearable whose scene.json copies differ', async () => {
      await expectItemFileError(
        loadItemFile(
          await zipFile({
            'male/scene.json': scene(),
            'male/bin/game.js': 'code',
            'male/model.glb': 'glb',
            'female/scene.json': scene({ requiredPermissions: ['USE_FETCH'] }),
            'female/bin/game.js': 'code',
            'female/model.glb': 'glb'
          })
        ),
        'scene_code_mismatch'
      )
    })

    it('requires the code in every body-shape folder when it is not at the root', async () => {
      const promise = loadItemFile(
        await zipFile({
          'male/scene.json': scene(),
          'male/bin/game.js': 'code',
          'male/model.glb': 'glb',
          'female/scene.json': scene(),
          'female/model.glb': 'glb'
        })
      )
      await expectItemFileError(promise, 'manifest_file_missing')
      await expect(promise).rejects.toMatchObject({ messageParams: { fileName: 'female/bin/game.js' } })
    })

    it('keeps a bundled video.mp4 without counting it toward the size cap', async () => {
      const result = await loadItemFile(
        await zipFile({
          'scene.json': scene(),
          'model.glb': 'glb',
          'bin/game.js': 'code',
          'video.mp4': blob(3 * 1024 * 1024)
        })
      )
      expect(result.contents['video.mp4']).toBeDefined()
    })

    it('still needs the code bundle when there is no wearable.json', async () => {
      const file = await zipFile({ 'scene.json': scene(), 'model.glb': 'glb' })
      await expectItemFileError(loadItemFile(file), 'manifest_file_missing')
    })

    it('needs the file scene.main points at', async () => {
      const file = await zipFile({ ...files, 'scene.json': scene({ main: 'bin/index.js' }) })
      await expectItemFileError(loadItemFile(file), 'manifest_file_missing')
    })

    it('rejects unknown, duplicated and hostname-less permissions with their own errors', async () => {
      await expectItemFileError(
        loadItemFile(await zipFile({ ...files, 'scene.json': scene({ requiredPermissions: ['FLY'] }) })),
        'unknown_required_permissions'
      )
      await expectItemFileError(
        loadItemFile(
          await zipFile({ ...files, 'scene.json': scene({ requiredPermissions: ['USE_FETCH', 'USE_FETCH'] }) })
        ),
        'duplicated_required_permissions'
      )
      await expectItemFileError(
        loadItemFile(
          await zipFile({ ...files, 'scene.json': scene({ requiredPermissions: ['ALLOW_MEDIA_HOSTNAMES'] }) })
        ),
        'allowed_media_hostnames_empty'
      )
    })

    it('rejects a scene.json missing its required fields', async () => {
      const file = await zipFile({ ...files, 'scene.json': JSON.stringify({ main: 'bin/game.js' }) })
      await expectItemFileError(loadItemFile(file), 'invalid_manifest')
    })

    it('keeps a preview video shipped in the zip as video.mp4, outside the model size cap', async () => {
      const result = await loadItemFile(
        await zipFile({ ...files, 'scene.json': scene(), 'media/showcase.mp4': blob(4 * 1024 * 1024) })
      )
      expect(Object.keys(result.contents).sort()).toEqual(['bin/game.js', 'glasses.glb', 'scene.json', 'video.mp4'])
      expect(result.contents['video.mp4'].size).toBe(4 * 1024 * 1024)
    })

    it('requires scene.main to point at the JS bundle', async () => {
      await expectItemFileError(
        loadItemFile(await zipFile({ ...files, 'scene.json': scene({ main: 'glasses.glb' }) })),
        'scene_main_not_code'
      )
    })

    it('rejects scene code shipped without a scene.json', async () => {
      await expectItemFileError(loadItemFile(await zipFile(files)), 'scene_manifest_missing')
    })

    it('rejects a zip with more than one video', async () => {
      await expectItemFileError(
        loadItemFile(await zipFile({ ...files, 'scene.json': scene(), 'a.mp4': 'a', 'b.mp4': 'b' })),
        'multiple_videos'
      )
    })

    it('drops a stray video from a plain wearable zip', async () => {
      const result = await loadItemFile(
        await zipFile({ 'wearable.json': wearableManifest, 'glasses.glb': 'glb', 'video.mp4': 'x' })
      )
      expect(Object.keys(result.contents)).toEqual(['glasses.glb'])
    })

    it('caps the model at the smart wearable size', async () => {
      const file = await zipFile({ ...files, 'glasses.glb': blob(3 * 1024 * 1024 + 1), 'scene.json': scene() })
      await expectItemFileError(loadItemFile(file), 'file_too_big')
    })
  })

  it('rejects zips whose only PNGs are orphaned auxiliaries', async () => {
    const file = await zipFile({ 'eyes.png': 'base', 'other_mask.png': 'mask' })
    await expectItemFileError(loadItemFile(file), 'orphaned_auxiliary_file')
  })

  it('rejects zips with no model at all', async () => {
    const file = await zipFile({ 'readme.txt': 'hello' })
    await expectItemFileError(loadItemFile(file), 'missing_model_file')
  })
})

describe('hasFacialExpressions', () => {
  it('is true only when an _expressions texture ships with the item', () => {
    expect(hasFacialExpressions({ 'eyes.png': blob(), 'eyes_expressions.png': blob() })).toBe(true)
    expect(hasFacialExpressions({ 'eyes.png': blob(), 'eyes_mask.png': blob() })).toBe(false)
  })
})

describe('validateVideoFile', () => {
  it('accepts an mp4 under the cap and rejects other formats or oversized files', () => {
    expect(() => validateVideoFile(new File([blob()], 'clip.MP4'))).not.toThrow()
    expect(() => validateVideoFile(new File([blob()], 'clip.mov'))).toThrow(ItemFileError)
    try {
      validateVideoFile(new File([blob(MAX_VIDEO_FILE_SIZE + 1)], 'clip.mp4'))
      expect.fail('expected an ItemFileError')
    } catch (error) {
      expect((error as ItemFileError).messageKey).toBe('video_too_big')
    }
  })
})

describe('rule book parity', () => {
  it('blocks thumbnails at the validator manifest size cap', () => {
    expect(MAX_THUMBNAIL_FILE_SIZE).toBe(manifest.fileSize.thumbnailBytes)
  })

  it('caps whole items at the validator manifest limits: 3 MB wearables and emotes, 9 MB skins', () => {
    expect(MAX_WEARABLE_FILE_SIZE).toBe(manifest.fileSize.wearableBytes)
    expect(MAX_EMOTE_FILE_SIZE).toBe(manifest.fileSize.emoteBytes)
    expect(MAX_SKIN_FILE_SIZE).toBe(manifest.fileSize.skinBytes)
  })
})
