import { mkdir, mkdtemp, readdir, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  atomicWrite,
  isAtomicTemporary,
  listFiles,
  readIfExists,
  resolveContained,
  validateOutputPath,
  canonicalPath
} from './fs-safe.js'

describe('fs-safe', () => {
  let root: string

  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'dcl-figma-guard-'))
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  describe('when writing an artifact atomically', () => {
    it('should create and replace the target without retaining a temporary file', async () => {
      const file = path.join(root, 'nested/artifact.json')
      await atomicWrite(file, 'old')
      await atomicWrite(file, 'new')
      expect(await readIfExists(file)).toBe('new')
      expect((await readdir(path.dirname(file))).some(isAtomicTemporary)).toBe(false)
    })
    it('should reject a destination link without changing its target', async () => {
      await writeFile(path.join(root, 'retained'), 'retained')
      await symlink(path.join(root, 'retained'), path.join(root, 'linked'))
      await expect(atomicWrite(path.join(root, 'linked'), 'changed')).rejects.toThrow('symlink')
      expect(await readIfExists(path.join(root, 'retained'))).toBe('retained')
    })
    it('should clean its temporary file if replacement fails', async () => {
      await mkdir(path.join(root, 'directory'))
      await expect(atomicWrite(path.join(root, 'directory'), 'changed')).rejects.toThrow()
      expect(await readdir(path.join(root, 'directory'))).toEqual([])
      expect((await readdir(root)).some(isAtomicTemporary)).toBe(false)
    })
    it('should ignore only reserved temporary names rather than legitimate JSON filenames', async () => {
      const temporary = '.figma-tmp-00000000-0000-4000-8000-000000000000'
      await writeFile(path.join(root, temporary), 'partial')
      await writeFile(path.join(root, `${temporary}.json`), '{}')
      expect(await listFiles(root)).toEqual([path.join(root, `${temporary}.json`)])
    })
  })
  describe('when a file is missing', () => {
    it('should report absence for a read and an empty list for a directory', async () => {
      expect(await readIfExists(path.join(root, 'nope.txt'))).toBeUndefined()
      expect(await listFiles(path.join(root, 'nope'))).toEqual([])
    })
  })

  describe('when the read fails for another reason', () => {
    it('should throw instead of reporting absence', async () => {
      await mkdir(path.join(root, 'a-directory'))

      await expect(readIfExists(path.join(root, 'a-directory'))).rejects.toThrow()
    })

    it('should throw when a file stands where a directory should be', async () => {
      await writeFile(path.join(root, 'file'), 'x')

      await expect(listFiles(path.join(root, 'file'))).rejects.toThrow()
    })
  })

  describe('when resolving a generated path', () => {
    it('should accept a nested relative path', async () => {
      expect(await resolveContained(root, 'components/a/B.tsx')).toBe(
        path.join(root, 'components/a/B.tsx')
      )
    })

    it.each(['../escape.ts', 'a/../../escape.ts', '/etc/passwd', '', 'a\0b'])(
      'should reject %j',
      async (bad) => {
        await expect(resolveContained(root, bad)).rejects.toThrow()
      }
    )

    it('should reject a path that goes through a symlink', async () => {
      const outside = await mkdtemp(path.join(os.tmpdir(), 'dcl-figma-outside-'))
      try {
        await symlink(outside, path.join(root, 'linked'))

        await expect(resolveContained(root, 'linked/file.ts')).rejects.toThrow(/symlink/)
      } finally {
        await rm(outside, { recursive: true, force: true })
      }
    })
  })
  describe('when validating an output directory', () => {
    it('should reject the workspace root itself', async () => {
      await expect(validateOutputPath(root, root)).rejects.toThrow('workspace root')
    })
    it('should reject a symlink ancestor that escapes the workspace', async () => {
      const outside = await mkdtemp(path.join(os.tmpdir(), 'outside-workspace-'))
      try {
        await symlink(outside, path.join(root, 'escape'))
        await expect(
          validateOutputPath(root, path.join(root, 'escape/new/output'))
        ).rejects.toThrow('outDir must stay inside')
      } finally {
        await rm(outside, { recursive: true, force: true })
      }
    })
    it('should accept a new descendant inside the workspace', async () => {
      expect(await validateOutputPath(root, path.join(root, 'new/output'))).toBe(
        await canonicalPath(path.join(root, 'new/output'))
      )
    })
  })
})
