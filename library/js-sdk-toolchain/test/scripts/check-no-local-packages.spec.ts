import { spawnSync } from 'child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'fs'
import { tmpdir } from 'os'
import { join, resolve } from 'path'

const SCRIPT = resolve(__dirname, '../../scripts/check-no-local-packages.cjs')

function check(manifest: Record<string, unknown>): number | null {
  const dir = mkdtempSync(join(tmpdir(), 'packed-'))
  try {
    writeFileSync(join(dir, 'package.json'), JSON.stringify(manifest))
    return spawnSync(process.execPath, [SCRIPT, dir]).status
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

describe('check-no-local-packages', () => {
  it('accepts a preview that aliases a sibling at its own version', () => {
    const status = check({
      name: '@dcl-regenesislabs/sdk',
      version: '7.0.0-1.commit-abc',
      dependencies: { '@dcl/ecs': 'npm:@dcl-regenesislabs/ecs@7.0.0-1.commit-abc' }
    })
    expect(status).toBe(0)
  })

  it('rejects a preview that aliases a sibling at another version', () => {
    const status = check({
      name: '@dcl-regenesislabs/sdk',
      version: '7.0.0-1.commit-abc',
      dependencies: { '@dcl/ecs': 'npm:@dcl-regenesislabs/ecs@7.0.0-2.commit-def' }
    })
    expect(status).toBe(1)
  })

  it('rejects a release that aliases a dependency', () => {
    const status = check({
      name: '@dcl/sdk',
      version: '7.34.0',
      dependencies: { '@dcl/ecs': 'npm:@dcl-regenesislabs/ecs@7.34.0' }
    })
    expect(status).toBe(1)
  })

  it('accepts a release that pins its siblings by version', () => {
    expect(check({ name: '@dcl/sdk', version: '7.34.0', dependencies: { '@dcl/ecs': '7.34.0' } })).toBe(0)
  })
})
