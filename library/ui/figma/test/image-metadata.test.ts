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

// Gate: no raster under library/ui/figma (the frozen test capture included), or in the public component
// assets, may carry non-rendering metadata (tEXt, iTXt, zTXt, eXIf, tIME). The public allowlist holds
// exactly the owner-approved NavBar assets; an entry that becomes clean must be removed.
const repoRoot = path.resolve(import.meta.dirname, '../../../..')
const PUBLIC_ASSETS = 'library/ui/components/assets/'
const EXPIRY = 'NavBar re-promotion'
// The 12 promoted NavBar assets under the owner-approved exception (2026-11-05). Changing this
// list is a scope decision, not a test fix.
const APPROVED_PUBLIC_EXCEPTIONS = [].map((name) => `${PUBLIC_ASSETS}${name}.svg`)

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

  it('should allow exactly the 12 owner-approved public NavBar assets, each with a reason and an expiry', async () => {
    const allow = await allowlist()

    // The exact approved set, not a count: a substituted or duplicated path fails here.
    expect(allow.files.map((entry) => entry.path)).toEqual(APPROVED_PUBLIC_EXCEPTIONS)
    expect(new Set(allow.files.map((entry) => entry.path)).size).toBe(allow.files.length)
    for (const entry of allow.files) {
      expect(entry.path.startsWith(PUBLIC_ASSETS) && entry.path.endsWith('.svg')).toBe(true)
      expect(entry.reason).toMatch(/NavBar/)
      expect(entry.reason).toMatch(/Owner-approved exception \(2026-11-05\)/)
      expect(entry.expires).toBe(EXPIRY)
    }
  })
})
