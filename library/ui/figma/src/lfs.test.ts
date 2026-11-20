import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { LfsPointerError, readMaterialized } from './lfs.js'
import { createRuntime, type Runtime } from './runtime.js'
import { pinTestCapture } from '../test/snapshot.js'

const POINTER = 'version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n'

let directory: string

beforeEach(async () => {
  directory = await mkdtemp(path.join(tmpdir(), 'lfs-test-'))
})

afterEach(async () => {
  await rm(directory, { recursive: true, force: true })
})

describe('readMaterialized', () => {
  let file: string

  beforeEach(() => {
    file = path.join(directory, 'a.json')
  })

  describe('when the file holds content', () => {
    beforeEach(async () => {
      await writeFile(file, '{}')
    })

    it('should return it', async () => {
      expect((await readMaterialized(file)).toString()).toBe('{}')
    })
  })

  describe('when the file is a Git LFS pointer', () => {
    beforeEach(async () => {
      await writeFile(file, POINTER)
    })

    it('should throw an LfsPointerError', async () => {
      await expect(readMaterialized(file)).rejects.toBeInstanceOf(LfsPointerError)
    })

    it('should name the file in the error message', async () => {
      await expect(readMaterialized(file)).rejects.toThrow(
        `${file} is a Git LFS pointer. Check out with LFS content (git lfs pull).`
      )
    })
  })
})

describe('the MCP runtime', () => {
  describe('when the saved capture is an unmaterialized LFS pointer', () => {
    let runtime: Runtime

    beforeEach(async () => {
      await mkdir(path.join(directory, 'snapshot/components'), { recursive: true })
      await writeFile(
        path.join(directory, 'figma.config.json'),
        JSON.stringify({
          outDir: 'generated',
          snapshotDir: 'snapshot',
          pages: { 'Nav bar': { kind: 'components' } },
          contract: { mappingRoot: 'components' }
        })
      )
      await writeFile(path.join(directory, 'snapshot/components/Nav bar.json'), POINTER)
      // A pointer that fetch-source would have pinned: the cache verifies, and reading it must still fail.
      await pinTestCapture(directory)

      runtime = await createRuntime({ cwd: directory, env: {} })
    })

    it('should fail instead of silently falling back to resolve-only mode', async () => {
      await expect(runtime.resolveContext()).rejects.toBeInstanceOf(LfsPointerError)
    })

    it('should name the pointer file in the error message', async () => {
      await expect(runtime.resolveContext()).rejects.toThrow(/Nav bar\.json is a Git LFS pointer/)
    })
  })
})
