import {
  access,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  realpath,
  rename,
  rm,
  writeFile
} from 'node:fs/promises'
import path from 'node:path'

// A headless browser can stall for reasons unrelated to the component: a preview that never signals it is
// ready, or a DevTools call that never answers. Only those timeouts are retried, at most once per variant
// and with at most one browser restart per run. A page exception or a wrong render is never retried.
class CaptureTimeout extends Error {}

function createRetryBudget({ restarts = 1 } = {}) {
  return { restartsLeft: restarts }
}

async function captureWithRetry(key, attempt, { budget, restart, log }) {
  try {
    return await attempt()
  } catch (error) {
    if (!(error instanceof CaptureTimeout)) throw error
    const restarting = budget.restartsLeft > 0
    log(`${key}: ${error.message}; retrying once${restarting ? ' in a new browser' : ''}`)
    if (restarting) {
      budget.restartsLeft -= 1
      await restart()
    }
  }
  try {
    return await attempt()
  } catch (error) {
    if (error instanceof CaptureTimeout)
      throw new Error(`${key}: capture timed out again after a retry: ${error.message}`, {
        cause: error
      })
    throw error
  }
}

// The capture talks to the browser through the global WebSocket, which older Node versions do not have.
function assertNodeVersion(required, actual) {
  const major = (version) => Number(String(version).trim().replace(/^v/, '').split('.')[0])
  if (major(actual) < major(required))
    throw new Error(
      `review:capture needs Node ${required.trim()} (from engines.node); this is Node ${actual}`
    )
}

// Published evidence is the pair renders/ + fidelity.json. A run builds the complete new pair in a staging
// folder: a copy of the live renders, so frozen evidence stays byte-identical, plus this run's captures.
// Publishing then swaps each piece through a backup under a commit journal. A run that fails before the
// journal leaves the live pair untouched. A commit interrupted after it is completed forward by the next run,
// and review:check refuses to pass while a journal is pending, so a mixed pair is never accepted.
const JOURNAL = '.capture-commit.json'
const BACKUP = '.capture-backup'
const PIECES = ['renders', 'fidelity.json']
const exists = (file) =>
  access(file).then(
    () => true,
    () => false
  )
const pendingCommit = (scratch) => exists(path.join(scratch, JOURNAL))

const STAGING_NAME = /^\.capture-staging-[A-Za-z0-9_-]+$/

// A folder the commit may rename into or delete: a real directory (no symlink) directly inside scratch, or
// absent. Anything else could point a recursive delete outside the capture area.
async function containedFolder(scratch, name) {
  const folder = path.join(scratch, name)
  const info = await lstat(folder).catch((error) => {
    if (error.code === 'ENOENT') return undefined
    throw error
  })
  if (info && (!info.isDirectory() || info.isSymbolicLink()))
    throw new Error(`Refusing the capture commit: ${name} is not a plain folder`)
  if (info && path.dirname(await realpath(folder)) !== (await realpath(scratch)))
    throw new Error(`Refusing the capture commit: ${name} resolves outside ${scratch}`)
  return folder
}

// The journal is read back from disk, so it is validated before it may name anything to move or delete.
async function readJournal(scratch, text) {
  let journal
  try {
    journal = JSON.parse(text)
  } catch {
    throw new Error(`Refusing the capture commit: ${JOURNAL} is not valid JSON`)
  }
  if (
    typeof journal !== 'object' ||
    journal === null ||
    Array.isArray(journal) ||
    Object.keys(journal).join() !== 'staging' ||
    typeof journal.staging !== 'string' ||
    !STAGING_NAME.test(journal.staging)
  )
    throw new Error(`Refusing the capture commit: ${JOURNAL} does not name a staging folder`)
  return {
    staging: await containedFolder(scratch, journal.staging),
    backup: await containedFolder(scratch, BACKUP)
  }
}

async function completeCommit({ output, scratch }, { staging, backup }, interrupt) {
  await mkdir(backup, { recursive: true })
  for (const piece of PIECES) {
    if (!(await exists(path.join(staging, piece)))) {
      if (!(await exists(path.join(output, piece))))
        throw new Error(`Interrupted capture commit lost ${piece}; restore it from git`)
      continue
    }
    if (await exists(path.join(output, piece))) {
      if (await exists(path.join(backup, piece)))
        throw new Error(
          `Interrupted capture commit is inconsistent at ${piece}; restore it from git`
        )
      await rename(path.join(output, piece), path.join(backup, piece))
    }
    await interrupt(`backup ${piece}`)
    await rename(path.join(staging, piece), path.join(output, piece))
    await interrupt(`publish ${piece}`)
  }
  await rm(backup, { recursive: true, force: true })
  await rm(staging, { recursive: true, force: true })
  await interrupt('cleanup')
  await rm(path.join(scratch, JOURNAL))
}

// Completes a commit an earlier run left behind, and drops staging or backup folders no journal refers to.
async function recoverInterruptedCommit({ output, scratch }, interrupt = async () => {}) {
  const journalFile = path.join(scratch, JOURNAL)
  if (await exists(journalFile))
    await completeCommit(
      { output, scratch },
      await readJournal(scratch, await readFile(journalFile, 'utf8')),
      interrupt
    )
  for (const entry of await readdir(scratch))
    if (entry.startsWith('.capture-staging-') || entry === BACKUP)
      await rm(path.join(scratch, entry), { recursive: true, force: true })
}

// `scratch` must be on the output's file system so each move is a rename. `interrupt` lets tests stop the
// publication after any step.
async function withStagedOutput({ output, scratch }, run, interrupt = async () => {}) {
  await recoverInterruptedCommit({ output, scratch })
  const staging = await mkdtemp(path.join(scratch, '.capture-staging-'))
  try {
    if (await exists(path.join(output, 'renders')))
      await cp(path.join(output, 'renders'), path.join(staging, 'renders'), { recursive: true })
    else await mkdir(path.join(staging, 'renders'))
    const result = await run(staging)
    if (!(await exists(path.join(staging, 'fidelity.json'))))
      throw new Error('The capture run finished without writing fidelity.json')
    const text = JSON.stringify({ staging: path.basename(staging) }) + '\n'
    const journal = await readJournal(scratch, text)
    await writeFile(path.join(scratch, JOURNAL + '.tmp'), text)
    await rename(path.join(scratch, JOURNAL + '.tmp'), path.join(scratch, JOURNAL))
    await interrupt('journal')
    await completeCommit({ output, scratch }, journal, interrupt)
    return result
  } finally {
    if (!(await pendingCommit(scratch))) await rm(staging, { recursive: true, force: true })
  }
}

export {
  CaptureTimeout,
  assertNodeVersion,
  captureWithRetry,
  createRetryBudget,
  pendingCommit,
  recoverInterruptedCommit,
  withStagedOutput
}
