import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import JSZip from 'jszip'
import { HASH } from '../fixtures'

const FILES = resolve(import.meta.dirname, '../fixtures/files')
const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

/**
 * Committed fixtures, all Decentraland base items (public), each checked against the wearable-validator rule book:
 * - hat: one model for both body shapes, passes.
 * - tshirtMale / tshirtFemale: an upper_body model per shape, passes with no hides.
 * - poloMale / poloFemale: an upper_body model per shape; both FAIL texture-size (a 2048px texture, cap 1024).
 * - hairSpringBones: female hair with DCL spring bones, passes.
 * - emote / emoteWithProps: pass looping or not; the second carries a prop armature and its own animation.
 * - emoteWithPropsAudio: the Money emote's sound (mp3), which passes the rule book's audio check with it.
 * - eyes / eyesMask: a 256×256 facial-feature texture and its mask. The rule book skips texture-only items.
 * - thumbnail: 1024×1024 PNG, passes the thumbnail rule.
 * - smartWearable: the hat as a smart wearable, packed by `sdk-commands pack-smart-wearable` (SDK 7.32,
 *   production build of a scene that spins a cube above the wearer). Its zip carries no video.
 * - previewVideo: a 3s 320×320 H.264 clip the e2e Chrome decodes.
 */
export const FIXTURE = {
  hat: join(FILES, 'hat.glb'),
  tshirtMale: join(FILES, 'tshirt-male.glb'),
  tshirtFemale: join(FILES, 'tshirt-female.glb'),
  poloMale: join(FILES, 'polo-male.glb'),
  poloFemale: join(FILES, 'polo-female.glb'),
  hairSpringBones: join(FILES, 'hair-spring-bones.glb'),
  emote: join(FILES, 'emote.glb'),
  emoteWithProps: join(FILES, 'emote-props.glb'),
  emoteWithPropsAudio: join(FILES, 'emote-props.mp3'),
  eyes: join(FILES, 'eyes.png'),
  eyesMask: join(FILES, 'eyes_mask.png'),
  thumbnail: join(FILES, 'thumbnail.png'),
  smartWearable: join(FILES, 'smart-wearable.zip'),
  previewVideo: join(FILES, 'preview.mp4')
} as const

const read = (path: string) => readFileSync(path)

/** The bytes behind every fixture content hash, served by the fake builder storage. */
export function fixtureStorage(): Record<string, Buffer> {
  return {
    [HASH.hat]: read(FIXTURE.hat),
    [HASH.tshirtMale]: read(FIXTURE.tshirtMale),
    [HASH.tshirtFemale]: read(FIXTURE.tshirtFemale),
    [HASH.poloMale]: read(FIXTURE.poloMale),
    [HASH.poloFemale]: read(FIXTURE.poloFemale),
    [HASH.emote]: read(FIXTURE.emote),
    [HASH.thumbnail]: read(FIXTURE.thumbnail)
  }
}

// One directory per run, so two worktrees running at once never overwrite each other's zips.
let outputDir: string | undefined
const outputPath = (name: string) => join((outputDir ??= mkdtempSync(join(tmpdir(), 'creators-e2e-'))), name)

async function writeZip(name: string, entries: Record<string, Buffer | string>): Promise<string> {
  const zip = new JSZip()
  for (const [path, content] of Object.entries(entries)) zip.file(path, content)
  const out = outputPath(name)
  writeFileSync(out, await zip.generateAsync({ type: 'nodebuffer' }))
  return out
}

/** The t-shirt pair in the legacy Builder's manifest format: wearable.json, a model per body shape, a thumbnail. */
export function legacyWearableZip(): Promise<string> {
  const representation = (bodyShape: string, file: string) => ({
    bodyShapes: [bodyShape],
    mainFile: file,
    contents: [file],
    overrideHides: [],
    overrideReplaces: []
  })
  const manifest = {
    name: 'Yellow T-Shirt',
    description: 'A t-shirt for e2e runs',
    rarity: 'common',
    data: {
      category: 'upper_body',
      hides: [],
      replaces: [],
      tags: ['tshirt'],
      representations: [representation(MALE, 'male/tshirt.glb'), representation(FEMALE, 'female/tshirt.glb')]
    }
  }
  return writeZip('legacy-wearable.zip', {
    'wearable.json': JSON.stringify(manifest, null, 2),
    'male/tshirt.glb': read(FIXTURE.tshirtMale),
    'female/tshirt.glb': read(FIXTURE.tshirtFemale),
    'thumbnail.png': read(FIXTURE.thumbnail)
  })
}

/** The eyes texture with its mask: a mask only travels with its texture inside a zip. */
export function eyesWithMaskZip(): Promise<string> {
  return writeZip('eyes-with-mask.zip', { 'eyes.png': read(FIXTURE.eyes), 'eyes_mask.png': read(FIXTURE.eyesMask) })
}

/** The Money emote with its sound, as a creator zips them: model and audio side by side. */
export function emoteWithAudioZip(): Promise<string> {
  return writeZip('emote-with-audio.zip', {
    'money.glb': read(FIXTURE.emoteWithProps),
    'money.mp3': read(FIXTURE.emoteWithPropsAudio)
  })
}

/** An emote as the app's "Download" exports it: both body shapes hold the same model, so it ships once at the root. */
export function downloadedEmoteZip(model: string = FIXTURE.emote): Promise<string> {
  return writeZip('downloaded-emote.zip', {
    'emote.glb': read(model),
    'thumbnail.png': read(FIXTURE.thumbnail),
    'image.png': read(FIXTURE.thumbnail)
  })
}

/** An emote download that keeps a folder per body shape: the model under male/ and female/, plus the thumbnail. */
export function downloadedEmoteFoldersZip(model: string = FIXTURE.emote): Promise<string> {
  const glb = read(model)
  return writeZip('downloaded-emote-folders.zip', {
    'male/emote.glb': glb,
    'female/emote.glb': glb,
    'thumbnail.png': read(FIXTURE.thumbnail)
  })
}

/** The SDK-packed smart wearable with the preview video inside, which pre-fills the import's video step. */
export async function smartWearableWithVideoZip(): Promise<string> {
  const zip = await JSZip.loadAsync(read(FIXTURE.smartWearable))
  zip.file('preview.mp4', read(FIXTURE.previewVideo))
  const out = outputPath('smart-wearable-with-video.zip')
  writeFileSync(out, await zip.generateAsync({ type: 'nodebuffer' }))
  return out
}
