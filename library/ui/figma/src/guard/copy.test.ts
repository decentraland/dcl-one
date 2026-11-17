import { mkdir, mkdtemp, readFile, rm, stat, symlink, utimes, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { copyManagedTree } from './copy.js'

describe('when copying managed artifacts into a stage', () => {
  let root: string
  let source: string
  let destination: string
  let retained: string
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'managed-copy-'))
    source = path.join(root, 'source')
    destination = path.join(root, 'destination')
    retained = path.join(root, 'retained.txt')
    await mkdir(source)
    await writeFile(retained, 'retained')
    await writeFile(path.join(source, 'component.tsx'), 'component')
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should copy ordinary artifacts', async () => {
    await copyManagedTree(source, destination)
    expect(await readFile(path.join(destination, 'component.tsx'), 'utf8')).toBe('component')
  })
  it('should preserve the timestamp of unchanged destination bytes', async () => {
    await copyManagedTree(source, destination)
    const file = path.join(destination, 'component.tsx')
    await utimes(file, new Date('2000-01-01'), new Date('2000-01-01'))
    const before = (await stat(file)).mtimeMs
    await copyManagedTree(source, destination)
    expect((await stat(file)).mtimeMs).toBe(before)
  })
  it('should reject source file links without changing their targets', async () => {
    await symlink(retained, path.join(source, 'linked.tsx'))
    await expect(copyManagedTree(source, destination)).rejects.toThrow('symlinks')
    expect(await readFile(retained, 'utf8')).toBe('retained')
  })
  it('should reject source directory links', async () => {
    await symlink(path.dirname(source), path.join(source, 'linked-directory'))
    await expect(copyManagedTree(source, destination)).rejects.toThrow('symlinks')
    expect(await readFile(retained, 'utf8')).toBe('retained')
  })
  it('should reject destination links before following them', async () => {
    await mkdir(destination)
    await symlink(retained, path.join(destination, 'component.tsx'))
    await expect(copyManagedTree(source, destination)).rejects.toThrow('symlinks')
    expect(await readFile(retained, 'utf8')).toBe('retained')
  })
  it('should leave missing-input errors visible to the caller', async () => {
    await expect(copyManagedTree(path.join(root, 'missing'), destination)).rejects.toMatchObject({
      code: 'ENOENT'
    })
  })
  it('should omit dependency and build directories without following their links', async () => {
    await symlink(path.dirname(source), path.join(source, 'node_modules'))
    await copyManagedTree(source, destination)
    await expect(
      readFile(path.join(destination, 'node_modules/retained.txt'))
    ).rejects.toMatchObject({ code: 'ENOENT' })
  })
})
