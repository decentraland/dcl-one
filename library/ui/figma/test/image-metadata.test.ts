import { execFileSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  captureRasters,
  fileMetadata,
  gateProblems,
  rastersIn,
  type Allowlist
} from '../src/capture/image-metadata-scan.js'
import { readMaterialized } from '../src/lfs.js'

// Tracked tooling and installed component assets must be free of non-rendering image metadata.
// The public mock assets and their metadata exceptions were retired together.
const repoRoot = path.resolve(import.meta.dirname, '../../../..')
const PUBLIC_ASSETS = 'library/ui/components/assets/'

// Tracked files plus new, not-ignored ones, so the gate also holds before a change is committed.
const files = execFileSync(
  'git',
  [
    'ls-files',
    '-z',
    '--cached',
    '--others',
    '--exclude-standard',
    'library/ui/figma',
    PUBLIC_ASSETS
  ],
  { cwd: repoRoot }
)
  .toString('utf8')
  .split('\0')
  .filter(Boolean)

const allowlist = async (): Promise<Allowlist> =>
  JSON.parse(
    await readFile(path.join(repoRoot, 'library/ui/figma/image-metadata-allow.json'), 'utf8')
  )

describe('image metadata gate', () => {
  it('should find no image metadata outside the allowlisted public NavBar assets', async () => {
    const found = new Map<string, Record<string, number>>()
    const undercounted: string[] = []
    for (const file of [...new Set(files)]) {
      let bytes: Buffer
      try {
        bytes = await readMaterialized(path.join(repoRoot, file))
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ENOENT') continue
        throw error
      }
      const counts = fileMetadata(file, bytes)
      if (Object.keys(counts).length) found.set(file, counts)
      // The schema walk must see every raster a blind field walk sees, so no location is skipped.
      if (/figma-snapshot\/components\/[^/]+\.json$/.test(file)) {
        const parsed = JSON.parse(bytes.toString('utf8'))
        undercounted.push(
          ...(rastersIn(file, bytes).length < captureRasters(parsed).length ? [file] : [])
        )
      }
    }

    expect(gateProblems(found, await allowlist(), PUBLIC_ASSETS)).toEqual([])
    expect(undercounted).toEqual([])
  }, 180_000)

  it('should require the actual image metadata allowlist to contain no public asset exception', async () => {
    expect((await allowlist()).files).toEqual([])
  })
})
