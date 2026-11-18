import { describe, expect, it } from 'vitest'
import JSZip from 'jszip'
import { buildItemZip, getItemZipName } from './itemDownload'
import { loadItemFile } from './itemFiles'
import { BodyShapeType, ItemType, type Item } from './items'

const item = { id: 'abc-1', name: 'My Hat!', type: ItemType.WEARABLE } as Item

/** Zips an item whose stored files are given as path → bytes; equal bytes are the same stored file. */
async function zipBlobOf(stored: Record<string, string>): Promise<Blob> {
  const contents = Object.fromEntries(Object.entries(stored).map(([path, bytes]) => [path, `hash-of-${bytes}`]))
  const files = Object.fromEntries(Object.entries(stored).map(([path, bytes]) => [path, new Blob([bytes])]))
  return buildItemZip({ contents }, files)
}

async function zipOf(stored: Record<string, string>): Promise<JSZip> {
  return JSZip.loadAsync(await (await zipBlobOf(stored)).arrayBuffer())
}

/** Downloads the item and imports the zip back. */
async function reimport(stored: Record<string, string>) {
  return loadItemFile(new File([await (await zipBlobOf(stored)).arrayBuffer()], 'item.zip'))
}

const filesIn = (zip: JSZip) =>
  Object.values(zip.files)
    .filter(entry => !entry.dir)
    .map(entry => entry.name)
    .sort()

describe('item download', () => {
  it('ships the files once, at the root, when both body shapes hold the same ones', async () => {
    const zip = await zipOf({
      'male/hat.glb': 'glb',
      'male/textures/hat.png': 'texture',
      'male/sound.mp3': 'audio',
      'female/hat.glb': 'glb',
      'female/textures/hat.png': 'texture',
      'female/sound.mp3': 'audio',
      'thumbnail.png': 'thumb',
      'image.png': 'catalyst'
    })
    expect(filesIn(zip)).toEqual(['hat.glb', 'image.png', 'sound.mp3', 'textures/hat.png', 'thumbnail.png'])
    expect(await zip.file('hat.glb')!.async('text')).toBe('glb')
    expect(await zip.file('thumbnail.png')!.async('text')).toBe('thumb')
  })

  it('ships a smart wearable with its scene and code at the root, next to the video', async () => {
    const zip = await zipOf({
      'male/scene.json': 'scene',
      'male/bin/game.js': 'code',
      'male/glasses.glb': 'glb',
      'female/scene.json': 'scene',
      'female/bin/game.js': 'code',
      'female/glasses.glb': 'glb',
      'thumbnail.png': 'thumb',
      'image.png': 'catalyst',
      'video.mp4': 'video'
    })
    expect(filesIn(zip)).toEqual([
      'bin/game.js',
      'glasses.glb',
      'image.png',
      'scene.json',
      'thumbnail.png',
      'video.mp4'
    ])
  })

  it('ships the model once, at the root, when the body shape folders are stored nested twice', async () => {
    const stored = {
      'male/male/dance.glb': 'glb',
      'male/female/dance.glb': 'glb',
      'female/male/dance.glb': 'glb',
      'female/female/dance.glb': 'glb',
      'thumbnail.png': 'thumb',
      'image.png': 'catalyst'
    }
    expect(filesIn(await zipOf(stored))).toEqual(['dance.glb', 'image.png', 'thumbnail.png'])
    expect((await reimport(stored)).model).toBe('dance.glb')
  })

  it('keeps the folder of an item made for one body shape', async () => {
    const zip = await zipOf({ 'male/hat.glb': 'glb', 'thumbnail.png': 'thumb' })
    expect(filesIn(zip)).toEqual(['male/hat.glb', 'thumbnail.png'])
    expect(await zip.file('male/hat.glb')!.async('text')).toBe('glb')
  })

  it('keeps both folders when any file differs between the body shapes', async () => {
    const stored = {
      'male/shirt.glb': 'glb-m',
      'male/shirt.png': 'texture',
      'female/shirt.glb': 'glb-f',
      'female/shirt.png': 'texture',
      'thumbnail.png': 'thumb'
    }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
  })

  it('keeps both folders when one body shape has a file the other lacks', async () => {
    const stored = { 'male/hat.glb': 'glb', 'male/extra.png': 'texture', 'female/hat.glb': 'glb' }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
  })

  it('keeps both folders when the same file goes by a different name in each body shape', async () => {
    const stored = { 'male/long.glb': 'glb', 'female/short.glb': 'glb', 'thumbnail.png': 'thumb' }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
  })

  it('keeps both folders when a file would land on a root file of the same name', async () => {
    const stored = { 'male/thumbnail.png': 'texture', 'female/thumbnail.png': 'texture', 'thumbnail.png': 'thumb' }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
  })

  it('keeps both folders when a file differs from a root file only by case', async () => {
    const stored = { 'male/Thumbnail.png': 'texture', 'female/Thumbnail.png': 'texture', 'thumbnail.png': 'thumb' }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
  })

  it("keeps both folders when a texture has the name of one of the item's own files", async () => {
    const stored = {
      'male/hat.glb': 'glb',
      'male/image.png': 'texture',
      'female/hat.glb': 'glb',
      'female/image.png': 'texture'
    }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
  })

  it('keeps both folders when two textures share a file name in different folders', async () => {
    const stored = {
      'male/hat.glb': 'glb',
      'male/a.png': 'texture-1',
      'male/textures/a.png': 'texture-2',
      'female/hat.glb': 'glb',
      'female/a.png': 'texture-1',
      'female/textures/a.png': 'texture-2'
    }
    expect(filesIn(await zipOf(stored))).toEqual(Object.keys(stored).sort())
    const imported = await reimport(stored)
    expect(imported.bodyShape).toBe(BodyShapeType.BOTH)
    expect(Object.keys(imported.contents).sort()).toEqual(Object.keys(stored).sort())
  })

  it('imports a folded download back with every file, its nested textures by their file name', async () => {
    const imported = await reimport({
      'male/hat.glb': 'glb',
      'male/textures/hat.png': 'texture',
      'male/sound.mp3': 'audio',
      'female/hat.glb': 'glb',
      'female/textures/hat.png': 'texture',
      'female/sound.mp3': 'audio',
      'thumbnail.png': 'thumb',
      'image.png': 'catalyst'
    })
    expect(imported.model).toBe('hat.glb')
    expect(imported.bodyShape).toBeNull()
    expect(Object.keys(imported.contents).sort()).toEqual([
      'hat.glb',
      'hat.png',
      'image.png',
      'sound.mp3',
      'thumbnail.png'
    ])
    expect(await imported.contents['hat.png'].text()).toBe('texture')
  })

  it('names the archive after the item', () => {
    expect(getItemZipName(item)).toBe('my-hat.zip')
    expect(getItemZipName({ ...item, name: '!!!' })).toBe('abc-1.zip')
  })
})
