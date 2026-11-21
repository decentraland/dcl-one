// @vitest-environment node
import { spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { hostedDigest } from '../scripts/hosted-digest.mjs'

const script = path.resolve(import.meta.dirname, '../scripts/hosted-digest.mjs')
let dir: string

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), 'hosted-digest-'))
  mkdirSync(path.join(dir, 'assets'))
  writeFileSync(path.join(dir, 'index.html'), '<!doctype html>')
  writeFileSync(path.join(dir, 'assets/index-a1.js'), 'export {}')
})
afterEach(() => rmSync(dir, { recursive: true, force: true }))

describe('when a hosted gallery build is digested', () => {
  it('should give the same digest for the same bytes and paths', () => {
    const copy = mkdtempSync(path.join(tmpdir(), 'hosted-digest-'))
    mkdirSync(path.join(copy, 'assets'))
    writeFileSync(path.join(copy, 'assets/index-a1.js'), 'export {}')
    writeFileSync(path.join(copy, 'index.html'), '<!doctype html>')
    expect(hostedDigest(copy)).toBe(hostedDigest(dir))
    rmSync(copy, { recursive: true })
  })

  it('should change when a byte, a file name or a file set changes', () => {
    const before = hostedDigest(dir)
    writeFileSync(path.join(dir, 'assets/index-a1.js'), 'export {};')
    const edited = hostedDigest(dir)
    expect(edited).not.toBe(before)
    writeFileSync(path.join(dir, 'assets/extra.css'), '')
    expect(hostedDigest(dir)).not.toBe(edited)
  })

  it('should fail on a link instead of following it', () => {
    symlinkSync(path.join(dir, 'index.html'), path.join(dir, 'assets/link.html'))
    expect(() => hostedDigest(dir)).toThrow('assets/link.html is not a regular file')
  })

  it('should fail on a directory that is not a gallery build', () => {
    rmSync(path.join(dir, 'index.html'))
    expect(() => hostedDigest(dir)).toThrow('holds no index.html')
  })
})

describe('when the digest script runs on a build', () => {
  it('should print the digest of the build', () => {
    const result = spawnSync('node', [script, dir], { encoding: 'utf8' })
    expect(result.status).toBe(0)
    expect(result.stdout.trim()).toBe(hostedDigest(dir))
  })

  it('should fail on a hidden file, which the artifact upload would drop unscanned', () => {
    writeFileSync(path.join(dir, 'assets/.vite-manifest.json'), '{}')
    const result = spawnSync('node', [script, dir], { encoding: 'utf8' })
    expect(result.status).toBe(1)
    expect(result.stderr).toContain('assets/.vite-manifest.json is hidden')
  })

  it('should fail on a hidden directory', () => {
    mkdirSync(path.join(dir, '.well-known'))
    writeFileSync(path.join(dir, '.well-known/x.txt'), 'x')
    expect(() => hostedDigest(dir)).toThrow('.well-known is hidden')
  })

  it('should refuse a missing or extra argument', () => {
    expect(spawnSync('node', [script], { encoding: 'utf8' }).status).toBe(2)
    expect(spawnSync('node', [script, dir, '--check', 'x'], { encoding: 'utf8' }).status).toBe(2)
  })
})
