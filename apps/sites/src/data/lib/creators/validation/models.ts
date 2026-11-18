import { type BodyShape } from '@dcl/schemas'
import { isImageFile } from '../itemFiles'
import { type Item, type ItemRepresentation } from '../items'
import { type ModelFile, type ValidationContext } from './types'

// The audio check (E-08) flags any of these; other content files (thumbnail, preview video) are never read.
export const AUDIO_LIKE = /\.(mp3|ogg|wav|aac|m4a|flac|opus|wma)$/i

/** A model to check: its main file, the body shapes that wear it and the audio files it ships with. */
export type DistinctModel = ModelFile & { audio: string[] }

/**
 * One entry per distinct main file, merging the body shapes of representations that share it (a unisex item
 * stores the same file under each body-shape folder). `identify` maps a path to what makes it the same file.
 * Texture-only representations have no geometry to inspect and are left out.
 */
export function distinctModels(
  representations: Pick<ItemRepresentation, 'mainFile' | 'bodyShapes' | 'contents'>[],
  identify: (path: string) => unknown = path => path
): DistinctModel[] {
  const models = new Map<unknown, DistinctModel>()
  for (const { mainFile, bodyShapes, contents } of representations) {
    if (isImageFile(mainFile)) continue
    const id = identify(mainFile) ?? mainFile
    const known = models.get(id)
    if (known) {
      for (const shape of bodyShapes)
        if (!known.bodyShapes.includes(shape as BodyShape)) known.bodyShapes.push(shape as BodyShape)
      continue
    }
    models.set(id, {
      mainFile,
      bodyShapes: [...bodyShapes] as BodyShape[],
      audio: (contents ?? []).filter(path => path !== mainFile && AUDIO_LIKE.test(path))
    })
  }
  return [...models.values()]
}

/** A saved item's distinct models, told apart by content hash. */
export function itemModels(item: Item): DistinctModel[] {
  return distinctModels(item.data.representations, path => item.contents[path])
}

/** What the checks read from an item besides its files. */
export function itemValidationContext(item: Pick<Item, 'type' | 'data'>): ValidationContext {
  return { type: item.type, category: item.data.category, hides: item.data.hides, loop: item.data.loop }
}
