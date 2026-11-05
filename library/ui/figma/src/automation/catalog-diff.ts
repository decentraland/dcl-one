import { execFile } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { atomicWrite, readIfExists } from '../guard/fs-safe.js'

const execute = promisify(execFile)
const catalogDiffFile = (reportFile: string) => `${reportFile}.catalog.diff`

async function saveCatalogDiff({
  before,
  stagedFile,
  reportFile
}: {
  before: string | undefined
  stagedFile: string
  reportFile: string
}): Promise<string | undefined> {
  const after = await readIfExists(stagedFile)
  if (after === undefined || after === before) return undefined
  const directory = await mkdtemp(path.join(os.tmpdir(), 'figma-catalog-diff-'))
  try {
    await writeFile(path.join(directory, 'before.json'), before ?? '')
    await writeFile(path.join(directory, 'staged.json'), after)
    const result = await execute(
      'git',
      [
        'diff',
        '--no-index',
        '--no-color',
        '--no-ext-diff',
        '--no-textconv',
        '--',
        'before.json',
        'staged.json'
      ],
      { cwd: directory, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }
    ).catch((error: unknown) => {
      const diff = error as { code?: number; stdout?: string }
      if (diff.code === 1 && typeof diff.stdout === 'string') return { stdout: diff.stdout }
      throw error
    })
    if (!result.stdout) return undefined
    const target = catalogDiffFile(reportFile)
    await atomicWrite(target, result.stdout)
    return path.basename(target)
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
}

export { catalogDiffFile, saveCatalogDiff }
