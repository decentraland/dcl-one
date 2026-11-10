import { execFile } from 'node:child_process'
import { rm } from 'node:fs/promises'
import { promisify } from 'node:util'
import { LEDGER } from './guard/consumers.js'
import { resolveContained } from './guard/fs-safe.js'
import { drainLedger, isAssetPath, type Ledger } from './guard/orphaned-assets.js'
import { LFS_POINTER, LfsPointerError } from './lfs.js'

const execFileAsync = promisify(execFile)

// The ledger as last committed, so a drain interrupted after dropping an entry still knows its file.
// Outside a git checkout, or before the first commit of the ledger, there is nothing to recover.
async function committedLedger(root: string): Promise<Ledger> {
  let stdout: string
  try {
    // HEAD:./ resolves against root, the library root (library/ui), not the repository root (layout.ts).
    // --filters runs the smudge filter: here the snapshot folder is stored in Git LFS (.gitattributes).
    ;({ stdout } = await execFileAsync('git', [
      '-C',
      root,
      'cat-file',
      '--filters',
      `HEAD:./${LEDGER}`
    ]))
  } catch {
    return { schema: 1, orphaned: [] }
  }
  // An unsmudged pointer would read as an empty ledger and turn every committed entry into a new one.
  if (stdout.startsWith(LFS_POINTER)) throw new LfsPointerError(`HEAD:${LEDGER}`)
  const parsed = JSON.parse(stdout) as Ledger
  return { schema: 1, orphaned: parsed.orphaned ?? [] }
}

async function drainOrphans(stagedDir: string, committed: Ledger, root: string) {
  return drainLedger(stagedDir, committed, root)
}

// Deletes drained files from the published tree. Their entries are removed afterwards by publication.
async function removeDrained(root: string, drained: string[]): Promise<void> {
  for (const file of drained) {
    if (!isAssetPath(file)) throw new Error(`Not a generated asset path: ${file}`)
    await rm(await resolveContained(root, file), { force: true })
  }
}

// What the drain is validated against: the committed ledger only. A drain interrupted after dropping an
// entry stays authorized by its committed record; an uncommitted entry never authorizes a deletion.
async function ledgerBase(_root: string, committed: Ledger): Promise<Ledger> {
  return {
    schema: 1,
    orphaned: [...committed.orphaned].sort((a, b) =>
      a.path < b.path ? -1 : a.path > b.path ? 1 : 0
    )
  }
}

export { committedLedger, drainOrphans, ledgerBase, removeDrained }
