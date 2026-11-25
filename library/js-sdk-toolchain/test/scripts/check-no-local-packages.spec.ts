import { spawnSync } from 'child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'fs'
import { tmpdir } from 'os'
import { join, resolve } from 'path'

const SCRIPT = resolve(__dirname, '../../scripts/check-no-local-packages.cjs')
const PREVIEW = ['--preview-scope', '@dcl-regenesislabs']
const VERSION = '7.0.0-1.commit-abc'

function check(manifest: Record<string, unknown>, args: string[] = []): number | null {
  const dir = mkdtempSync(join(tmpdir(), 'packed-'))
  try {
    writeFileSync(join(dir, 'package.json'), JSON.stringify(manifest))
    return spawnSync(process.execPath, [SCRIPT, dir, ...args]).status
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

function withEcs(name: string, version: string, ecs: string) {
  return { name, version, dependencies: { '@dcl/ecs': ecs } }
}

describe('check-no-local-packages', () => {
  describe('when the build is a preview', () => {
    it('accepts a renamed package that aliases a sibling at its own version', () => {
      const manifest = withEcs('@dcl-regenesislabs/sdk', VERSION, `npm:@dcl-regenesislabs/ecs@${VERSION}`)
      expect(check(manifest, PREVIEW)).toBe(0)
    })

    it('accepts a package that keeps its name, as the unpublished playground-assets does', () => {
      const manifest = withEcs('@dcl/playground-assets', VERSION, `npm:@dcl-regenesislabs/ecs@${VERSION}`)
      expect(check(manifest, PREVIEW)).toBe(0)
    })

    it('rejects an alias to a sibling at another version', () => {
      const manifest = withEcs('@dcl-regenesislabs/sdk', VERSION, 'npm:@dcl-regenesislabs/ecs@7.0.0-2.commit-def')
      expect(check(manifest, PREVIEW)).toBe(1)
    })

    it('rejects an alias to another scope', () => {
      const manifest = withEcs('@dcl-regenesislabs/sdk', VERSION, `npm:@other/ecs@${VERSION}`)
      expect(check(manifest, PREVIEW)).toBe(1)
    })
  })

  describe('when the build is a release', () => {
    it('rejects an alias', () => {
      expect(check(withEcs('@dcl/sdk', '7.34.0', 'npm:@dcl-regenesislabs/ecs@7.34.0'))).toBe(1)
    })

    it('accepts siblings pinned by version', () => {
      expect(check(withEcs('@dcl/sdk', '7.34.0', '7.34.0'))).toBe(0)
    })
  })
})
