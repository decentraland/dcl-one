import { execFile } from 'node:child_process'
import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'
import { listFiles, toPosix } from './fs-safe.js'

// Whether anything still uses a generated asset, decided over every text file of the tree that would be
// published. A path-resolvable reference counts only when it resolves to the asset itself; a bare hash
// that cannot be resolved counts, so an unknown kind of reference keeps the asset alive (fail closed).

const TEXT_FILE = /\.(?:ts|tsx|js|jsx|mjs|cjs|css|scss|html|json|md)$/
const SKIPPED_DIRECTORIES = new Set([
  'node_modules',
  '.git',
  'dist',
  'review-dist',
  'coverage',
  '.figma-cache'
])
const LEDGER = 'figma/figma-snapshot/orphaned-assets.json'
const RETIREMENTS = 'figma/figma-snapshot/retirements.json'
// Lifecycle bookkeeping lists the assets it tracks; it never uses them.
const BOOKKEEPING = new Set([LEDGER, RETIREMENTS])
// Integrity manifests fingerprint every generated file by path: only those fingerprint pairs are excluded.
const FINGERPRINT_MANIFESTS = new Set([
  'figma/review/src/review/promotion.json',
  'components/promotion.json'
])

// `paths` is every file of the tree (assets included), `texts` the contents of the text files.
type Tree = { paths: Set<string>; texts: Map<string, string> }

const decoder = new TextDecoder('utf-8', { fatal: true })

async function readText(root: string, relative: string): Promise<string> {
  try {
    return decoder.decode(await readFile(path.join(root, relative)))
  } catch (error) {
    throw new Error(`Cannot read ${relative} to check asset consumers: ${String(error)}`, {
      cause: error
    })
  }
}

const execFileAsync = promisify(execFile)

// A git checkout lists its tracked files plus new unignored ones; a stage without .git is walked.
async function listTree(root: string): Promise<string[]> {
  const isGit = await stat(path.join(root, '.git')).then(
    () => true,
    () => false
  )
  if (isGit) {
    const git = async (...args: string[]) =>
      (
        await execFileAsync('git', ['-C', root, 'ls-files', '-z', ...args], {
          maxBuffer: 64 * 1024 * 1024
        })
      ).stdout
        .split('\0')
        .filter(Boolean)
    // Only a tracked file Git itself reports as deleted leaves the tree; any other missing or unreadable
    // file fails the scan.
    const deleted = new Set(await git('--deleted'))
    const listed = (await git('--cached', '--others', '--exclude-standard')).filter(
      (file) => !deleted.has(file)
    )
    for (const file of listed)
      await stat(path.join(root, file)).catch((error: unknown) => {
        throw new Error(`Cannot stat ${file} to check asset consumers: ${String(error)}`, {
          cause: error
        })
      })
    return listed
  }
  return (await listFiles(root))
    .map((file) => toPosix(path.relative(root, file)))
    .filter((file) => !file.split('/').some((segment) => SKIPPED_DIRECTORIES.has(segment)))
}

async function readTree(root: string): Promise<Tree> {
  const paths = new Set(await listTree(root))
  const texts = new Map<string, string>()
  for (const file of paths) if (TEXT_FILE.test(file)) texts.set(file, await readText(root, file))
  return { paths, texts }
}

// The tree a partial stage publishes: base files, replaced or extended by everything staged. A staged
// deletion is not modelled, which can only keep more consumers (the safe direction).
async function effectiveTree(baseDir: string, stagedDir?: string): Promise<Tree> {
  const tree = await readTree(baseDir)
  if (!stagedDir || path.resolve(stagedDir) === path.resolve(baseDir)) return tree
  const staged = await readTree(stagedDir)
  for (const file of staged.paths) tree.paths.add(file)
  for (const [file, text] of staged.texts) tree.texts.set(file, text)
  return tree
}

const PATH_CHAR = /[A-Za-z0-9_.\-/@~%+]/

// The path-like token around one occurrence of the hash.
function tokenAt(text: string, start: number, end: number): string {
  let left = start
  let right = end
  while (left > 0 && PATH_CHAR.test(text[left - 1]!)) left--
  while (right < text.length && PATH_CHAR.test(text[right]!)) right++
  return text.slice(left, right)
}

// A `files` entry pairs a path with the sha256 of that file. Generated asset names are a sha256 prefix of
// their bytes, so the fingerprint value of an asset's own entry contains its name: the pair is excluded
// whole. A value that is not a sha256 is scanned like any other text.
const FINGERPRINT = /^[0-9a-f]{64}$/

function withoutFingerprintKeys(text: string): string {
  try {
    const data = JSON.parse(text) as Record<string, unknown>
    const files = data.files
    if (files && typeof files === 'object' && !Array.isArray(files))
      return JSON.stringify({
        ...data,
        files: Object.fromEntries(
          Object.entries(files).filter(
            ([, value]) => !(typeof value === 'string' && FINGERPRINT.test(value))
          )
        )
      })
  } catch {
    /* An unparseable manifest is scanned whole. */
  }
  return text
}

const SCRIPT = /\.(?:ts|tsx|js|jsx|mjs|cjs)$/

// Where a reference token points, or undefined when that cannot be decided.
function resolveToken(file: string, token: string, tree: Tree): string | undefined {
  const clean = token.replace(/[?#].*$/, '')
  const directory = path.posix.dirname(file)
  if (clean.startsWith('./') || clean.startsWith('../'))
    return path.posix.normalize(path.posix.join(directory, clean))
  // A bare file name or hash has no defined base: it counts, even beside a copy of the same name.
  if (!clean.includes('/')) return undefined
  // A package specifier in code, or a root-relative URL, has no repository meaning.
  if (SCRIPT.test(file) || clean.startsWith('/')) return undefined
  // Other text files resolve a plain relative path against themselves, a repository path as is.
  if (tree.paths.has(clean)) return clean
  return path.posix.normalize(path.posix.join(directory, clean))
}

// Every file that uses the asset, with the token that does so. Each occurrence is judged on its own: a
// reference resolved elsewhere (a library CSS URL to its own copy) never hides another in the same file.
function findConsumers(tree: Tree, asset: string): { file: string; token: string }[] {
  const basename = path.posix.basename(asset)
  const hash = basename.replace(/\.[^.]+$/, '')
  const found: { file: string; token: string }[] = []
  for (const [file, raw] of tree.texts) {
    if (file === asset || BOOKKEEPING.has(file)) continue
    const text = FINGERPRINT_MANIFESTS.has(file) ? withoutFingerprintKeys(raw) : raw
    for (let index = text.indexOf(hash); index !== -1; index = text.indexOf(hash, index + 1)) {
      const token = tokenAt(text, index, index + hash.length)
      // A bare hash (no extension) cannot be resolved and counts; so does an unresolvable path.
      const resolved = token.endsWith(basename) ? resolveToken(file, token, tree) : undefined
      if (resolved === undefined || resolved === asset) {
        found.push({ file, token })
        break
      }
    }
  }
  return found
}

export { LEDGER, RETIREMENTS, effectiveTree, findConsumers, readTree }
export type { Tree }
