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
    describe('and the target is written twice', () => {
      let file: string

      beforeEach(async () => {
        file = path.join(root, 'nested/artifact.json')
        await atomicWrite(file, 'old')
        await atomicWrite(file, 'new')
      })

      it('should replace the target with the latest content', async () => {
        expect(await readIfExists(file)).toBe('new')
      })

      it('should not retain a temporary file', async () => {
        expect((await readdir(path.dirname(file))).some(isAtomicTemporary)).toBe(false)
      })
    })

    describe('and the destination is a link', () => {
      let write: Promise<void>

      beforeEach(async () => {
        await writeFile(path.join(root, 'retained'), 'retained')
        await symlink(path.join(root, 'retained'), path.join(root, 'linked'))
        write = atomicWrite(path.join(root, 'linked'), 'changed')
        write.catch(() => undefined)
      })

      it('should reject it', async () => {
        await expect(write).rejects.toThrow(
          `Path goes through a symlink: ${path.join(root, 'linked')}`
        )
      })

      it('should not change the link target', async () => {
        await write.catch(() => undefined)
        expect(await readIfExists(path.join(root, 'retained'))).toBe('retained')
      })
    })

    describe('and the replacement fails', () => {
      let write: Promise<void>

      beforeEach(async () => {
        await mkdir(path.join(root, 'directory'))
        write = atomicWrite(path.join(root, 'directory'), 'changed')
        write.catch(() => undefined)
      })

      it('should reject with the rename error', async () => {
        await expect(write).rejects.toThrow(/^E[A-Z]+: .*rename/)
      })

      it('should leave the directory standing at the target empty', async () => {
        await write.catch(() => undefined)
        expect(await readdir(path.join(root, 'directory'))).toEqual([])
      })

      it('should clean its temporary file', async () => {
        await write.catch(() => undefined)
        expect((await readdir(root)).some(isAtomicTemporary)).toBe(false)
      })
    })

    describe('and the directory holds a reserved temporary name and a similar JSON file', () => {
      let temporary: string

      beforeEach(async () => {
        temporary = '.figma-tmp-00000000-0000-4000-8000-000000000000'
        await writeFile(path.join(root, temporary), 'partial')
        await writeFile(path.join(root, `${temporary}.json`), '{}')
      })

      it('should list every file but the reserved temporary name', async () => {
        expect(await listFiles(root)).toEqual([path.join(root, `${temporary}.json`)])
      })
    })
  })

  describe('when a file is missing', () => {
    it('should report absence for a read', async () => {
      expect(await readIfExists(path.join(root, 'nope.txt'))).toBeUndefined()
    })

    it('should report an empty list for a directory', async () => {
      expect(await listFiles(path.join(root, 'nope'))).toEqual([])
    })
  })

  describe('when the read fails for another reason', () => {
    describe('and the read targets a directory', () => {
      beforeEach(async () => {
        await mkdir(path.join(root, 'a-directory'))
      })

      it('should throw instead of reporting absence', async () => {
        await expect(readIfExists(path.join(root, 'a-directory'))).rejects.toThrow(/^EISDIR/)
      })
    })

    describe('and a file stands where a directory should be', () => {
      beforeEach(async () => {
        await writeFile(path.join(root, 'file'), 'x')
      })

      it('should throw instead of listing nothing', async () => {
        await expect(listFiles(path.join(root, 'file'))).rejects.toThrow(/^ENOTDIR/)
      })
    })
  })

  describe('when resolving a generated path', () => {
    it('should accept a nested relative path', async () => {
      expect(await resolveContained(root, 'components/a/B.tsx')).toBe(
        path.join(root, 'components/a/B.tsx')
      )
    })

    it.each([
      ['../escape.ts', 'Path escapes its root: ../escape.ts'],
      ['a/../../escape.ts', 'Path escapes its root: a/../../escape.ts'],
      ['/etc/passwd', 'Path is not contained: "/etc/passwd"'],
      ['', 'Path is not contained: ""'],
      ['a\0b', 'Path is not contained: "a\\u0000b"']
    ])('should reject %j', async (bad, message) => {
      await expect(resolveContained(root, bad)).rejects.toThrow(message)
    })

    describe('and the path goes through a symlink', () => {
      let outside: string

      beforeEach(async () => {
        outside = await mkdtemp(path.join(os.tmpdir(), 'dcl-figma-outside-'))
        await symlink(outside, path.join(root, 'linked'))
      })

      afterEach(async () => {
        await rm(outside, { recursive: true, force: true })
      })

      it('should reject it', async () => {
        await expect(resolveContained(root, 'linked/file.ts')).rejects.toThrow(
          'Path goes through a symlink: linked/file.ts'
        )
      })
    })
  })

  describe('when validating an output directory', () => {
    describe('and it is the workspace root itself', () => {
      it('should reject it', async () => {
        await expect(validateOutputPath(root, root)).rejects.toThrow('workspace root')
      })
    })

    describe('and a symlink ancestor escapes the workspace', () => {
      let outside: string

      beforeEach(async () => {
        outside = await mkdtemp(path.join(os.tmpdir(), 'outside-workspace-'))
        await symlink(outside, path.join(root, 'escape'))
      })

      afterEach(async () => {
        await rm(outside, { recursive: true, force: true })
      })

      it('should reject it', async () => {
        await expect(
          validateOutputPath(root, path.join(root, 'escape/new/output'))
        ).rejects.toThrow('outDir must stay inside')
      })
    })

    describe('and it is a new descendant inside the workspace', () => {
      it('should accept it', async () => {
        expect(await validateOutputPath(root, path.join(root, 'new/output'))).toBe(
          await canonicalPath(path.join(root, 'new/output'))
        )
      })
    })
  })
})
