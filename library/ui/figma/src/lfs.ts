import { readFile } from 'node:fs/promises'

const LFS_POINTER = 'version https://git-lfs'

class LfsPointerError extends Error {
  constructor(file: string) {
    super(`${file} is a Git LFS pointer. Check out with LFS content (git lfs pull).`)
    this.name = 'LfsPointerError'
  }
}

/**
 * Reads a file the tooling hashes, parses or ships. A Git LFS pointer means the checkout skipped
 * LFS content; hashing or parsing it would silently produce wrong output, so it fails loudly.
 */
async function readMaterialized(file: string): Promise<Buffer> {
  const content = await readFile(file)
  if (content.subarray(0, LFS_POINTER.length).toString('latin1') === LFS_POINTER)
    throw new LfsPointerError(file)
  return content
}

export { LFS_POINTER, LfsPointerError, readMaterialized }
