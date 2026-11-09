import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { scanBuffer } from '../key-guard.mjs'

// The Review publishes generated vector artwork only: an image fill (an SVG that embeds a raster) is replaced by
// a neutral placeholder at generation time, so every asset file the generated CSS references must be pure vector.
const generated = path.resolve(import.meta.dirname, '../review/src/generated/components')
const assets = path.join(generated, 'assets')
const LOCK = path.resolve(import.meta.dirname, '../../upstream.lock.json')
const LOCK_PREFIX = 'library/ui/figma/review/src/generated/components/assets/'

// Exactly the pure-vector assets the generator keeps once image fills become a placeholder, by name and sha256.
// Adding, dropping or changing one, or an image fill that slips back in as an asset, fails here first.
const EXPECTED = new Map<string, string>([
  ['120c99e4bbfb6775.svg', '120c99e4bbfb6775f6d27321e464cafd87e21b581fd21fccb0328f6f84b82d9c'],
  ['4151a1a77e219230.svg', '4151a1a77e219230aad292142ae460d9a9e744b446d57e203947d58bf7cb937d'],
  ['4d1c41d43547c45f.svg', '4d1c41d43547c45f9f9587d1ce47859b0b8a029cb1d6e44012fd3ce757bf2c45'],
  ['5054e13b50dac935.svg', '5054e13b50dac935ba79b6f3b474119c14524384a0d031d71cde19628b0fc0ef'],
  ['5df316c4042f7af1.svg', '5df316c4042f7af1621ccb0dd3663a19fa3faf17d63091447058564a04bd19dc'],
  ['730ae433766298b6.svg', '730ae433766298b6453e7ebaae6f6f3206b6b66ab7cfdc0c076732740a601f13'],
  ['797811dc24b080ee.svg', '797811dc24b080ee5f5dc54c4ad8fec5e0e1b0366a3b9306a64bb4ed169635eb'],
  ['7d732b5bf03864b7.svg', '7d732b5bf03864b766a86a1f6b2bff7d7142f545e60205160eeb90a96b871e1b'],
  ['863e8082847b755a.svg', '863e8082847b755ae8eb5b5784381e524ab91a7ff412d53384937ee4bd956c90'],
  ['89b1f6eedbc9735e.svg', '89b1f6eedbc9735e9ab0c84cbb7a40d1456ef7691c56c9a1dab29a4735086995'],
  ['92aa435762455ae2.svg', '92aa435762455ae2fea8a3a92981b4a99ba9c28910171c97183f5ef3d7a4b709'],
  ['9465e741066da76b.svg', '9465e741066da76b5bc5909a3bfa635aecfa360858fd0da687a7654adc2debd1'],
  ['9fde78005aa3e28d.svg', '9fde78005aa3e28d184d76a30a71ae1ef46d8fa91817d714d072ba24cb9976fe'],
  ['a6bd0d79c1d0bf6e.svg', 'a6bd0d79c1d0bf6ed65229c99582d4ea4cb8762fec1fa3c775dd5e17fb0d9def'],
  ['b8b742561a8f3bc8.svg', 'b8b742561a8f3bc8433e41ccf68110d5008804fdb118d6ddc7bdbac33b7f1c9b'],
  ['c5dd0513aa5460cd.svg', 'c5dd0513aa5460cd94971ed38f7d53c6d91a0d92891fcef9f994fec87f337898'],
  ['c6964a7df79861bd.svg', 'c6964a7df79861bdff65fdb72c0ee728d204b59b84da514958288375ca24b4b1'],
  ['e3411ff97e17fed4.svg', 'e3411ff97e17fed4873067b538946569ec15a248aaac064490b85361d58169f7'],
  ['e667e648244e4a58.svg', 'e667e648244e4a5813b29f5bcf18d1c7113d20b7bdacaa85c866d608e97d9d39'],
  ['e6bc80bec3a1d514.svg', 'e6bc80bec3a1d514ce95fce69433ff89d6b13983e63d7cf7a0977d5d5a50eebb'],
  ['e8801b56cc5074b8.svg', 'e8801b56cc5074b8b7f47390f274fa0508a44b22d34726aa2c417b407349e7ff'],
  ['f8b32fd93ff7a9d2.svg', 'f8b32fd93ff7a9d2aba6c532994eca98bc6927d54922aa82aab9744f7f047ef6'],
  ['fa98fc4666e86c99.svg', 'fa98fc4666e86c99827bfd7a4d61df59409975bf7b6115ad2c92bd6b66841f0f']
])

async function stylesheets(dir: string): Promise<string[]> {
  const out: string[] = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name)
    if (entry.isDirectory()) out.push(...(await stylesheets(file)))
    else if (entry.name.endsWith('.module.css')) out.push(file)
  }
  return out
}

async function referencedAssets(): Promise<Set<string>> {
  const names = new Set<string>()
  for (const file of await stylesheets(generated))
    for (const match of (await readFile(file, 'utf8')).matchAll(/url\("\.\.\/assets\/([^"]+)"\)/g))
      names.add(match[1]!)
  return names
}

// Reasons a vector asset would not be plain generated artwork.
function contentProblems(svg: string): string[] {
  const problems: string[] = []
  if (/<image\b/i.test(svg)) problems.push('embeds an <image>')
  if (/data:image\//i.test(svg)) problems.push('embeds a data:image URI')
  if (/\b(?:xlink:)?href\s*=\s*["'](?!#)/i.test(svg)) problems.push('links an external resource')
  if (/<metadata\b|figma\.com|data-figma|inkscape:|sodipodi:/i.test(svg))
    problems.push('carries editor or Figma metadata')
  return problems
}

describe('the generated Review assets', () => {
  it('should hold exactly the pinned vector assets, each with its pinned and locked digest', async () => {
    const present = existsSync(assets)
      ? (await readdir(assets)).filter((name) => !name.startsWith('.')).sort()
      : []
    const lock = JSON.parse(await readFile(LOCK, 'utf8')) as {
      sources: { repository: string; files?: Record<string, { sha256: string }> }[]
    }
    const files =
      lock.sources.find((source) => source.repository.endsWith('/figma-design-system'))?.files ?? {}
    const locked = Object.keys(files)
      .filter((file) => file.startsWith(LOCK_PREFIX))
      .map((file) => file.slice(LOCK_PREFIX.length))
      .sort()

    expect(EXPECTED.size).toBe(23)
    expect(present).toEqual([...EXPECTED.keys()].sort())
    expect(locked).toEqual([...EXPECTED.keys()].sort())
    for (const [name, digest] of EXPECTED) {
      const actual = createHash('sha256')
        .update(await readFile(path.join(assets, name)))
        .digest('hex')
      expect(actual, name).toBe(digest)
      expect(files[LOCK_PREFIX + name]?.sha256, name).toBe(digest)
    }
  })

  it('should resolve every asset the generated CSS references, and hold no unreferenced file', async () => {
    const referenced = await referencedAssets()
    const present = existsSync(assets)
      ? (await readdir(assets)).filter((name) => name.endsWith('.svg'))
      : []

    expect([...referenced].filter((name) => !present.includes(name))).toEqual([])
    expect(present.filter((name) => !referenced.has(name))).toEqual([])
  })

  it('should keep every referenced asset pure vector, with no link, metadata or Figma URL', async () => {
    for (const name of await referencedAssets()) {
      const bytes = await readFile(path.join(assets, name))
      expect(contentProblems(bytes.toString('utf8')), name).toEqual([])
      expect(scanBuffer(bytes, process.env.FIGMA_FILE_KEY), name).toEqual([])
    }
  })

  it('should recognize each content the rule refuses', () => {
    expect(contentProblems('<svg><image href="x.png"/></svg>')).toContain('embeds an <image>')
    expect(contentProblems('<svg><path fill="url(data:image/png;base64,AA)"/></svg>')).toContain(
      'embeds a data:image URI'
    )
    expect(contentProblems('<svg><use href="https://example.com/a.svg#b"/></svg>')).toContain(
      'links an external resource'
    )
    expect(contentProblems('<svg><use href="#local"/></svg>')).toEqual([])
    expect(contentProblems('<svg><metadata>x</metadata></svg>')).toContain(
      'carries editor or Figma metadata'
    )
    expect(contentProblems('<svg><path d="M0 0h1v1z"/></svg>')).toEqual([])
  })
})
