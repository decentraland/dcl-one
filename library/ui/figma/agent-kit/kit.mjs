import { createHash } from 'node:crypto'
import {
  cp,
  lstat,
  mkdir,
  readFile,
  readdir,
  readlink,
  rm,
  symlink,
  writeFile
} from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const defaultKitRoot = path.dirname(fileURLToPath(import.meta.url))
const MANIFEST = '.kit.json'
// The only .agents/skills this kit creates or accepts: a relative link to the installed skills.
const AGENTS_LINK_TARGET = '../.claude/skills'

// The kit ships in library/ui/figma/agent-kit; its version is the library package's.
async function readLibrary(kitRoot) {
  const pkg = JSON.parse(await readFile(path.join(kitRoot, '../..', 'package.json'), 'utf8'))
  return { name: pkg.name, version: pkg.version }
}

async function listFiles(dir, base = dir) {
  const files = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) files.push(...(await listFiles(full, base)))
    else if (entry.name !== MANIFEST) files.push(path.relative(base, full))
  }
  return files.sort()
}

async function hashDirectory(dir) {
  const hash = createHash('sha256')
  for (const file of await listFiles(dir)) {
    hash.update(file)
    hash.update(await readFile(path.join(dir, file)))
  }
  return hash.digest('hex')
}

async function exists(target) {
  try {
    await lstat(target)
    return true
  } catch {
    return false
  }
}

function compareVersions(a, b) {
  const pa = a.split('.').map(Number)
  const pb = b.split('.').map(Number)
  for (let i = 0; i < 3; i += 1) {
    if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) - (pb[i] ?? 0)
  }
  return 0
}

// absent, linked (a symlink that resolves to <target>/.claude/skills, however it is spelled, so a Windows
// `..\\.claude\\skills` counts), or conflict: anything else, including a real directory, a symlinked .agents that
// would place the link outside the target, or an .agents that is a file.
async function agentsLinkState(target) {
  const agents = path.join(target, '.agents')
  const link = path.join(agents, 'skills')
  const parent = await lstat(agents).catch((error) => {
    if (error.code === 'ENOENT') return undefined
    throw error
  })
  if (parent && !parent.isDirectory()) return { link, state: 'conflict', symlink: false }
  let stat
  try {
    stat = await lstat(link)
  } catch (error) {
    if (error.code === 'ENOENT') return { link, state: 'absent' }
    if (error.code === 'ENOTDIR') return { link, state: 'conflict', symlink: false }
    throw error
  }
  if (
    stat.isSymbolicLink() &&
    path.resolve(agents, await readlink(link)) === path.resolve(target, '.claude', 'skills')
  )
    return { link, state: 'linked' }
  return { link, state: 'conflict', symlink: stat.isSymbolicLink() }
}

async function skillNames(kitRoot) {
  const entries = await readdir(path.join(kitRoot, 'skills'), { withFileTypes: true })
  // Sorted, so install results and checks do not depend on the file system's listing order.
  return entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort()
}

// Copies the skills of this library version into <target>/.claude/skills. A skill directory the kit did
// not create (no manifest) belongs to the consumer and is never overwritten without --force. A skill that
// already matches this version byte for byte is left untouched, so a repeated install is a no-op.
// The .agents/skills link is opt-in; an existing entry that is not the kit's link is a conflict, refused
// before anything is written; --force replaces a foreign symlink but never removes a real directory.
async function installKit({ target, force = false, linkAgents = false, kitRoot = defaultKitRoot }) {
  const library = await readLibrary(kitRoot)
  const skillsDir = path.join(target, '.claude', 'skills')
  const installed = []
  const unchanged = []
  const agents = linkAgents ? await agentsLinkState(target) : undefined
  if (agents?.state === 'conflict' && (!force || !agents.symlink))
    throw new Error(
      agents.symlink
        ? `${agents.link} links elsewhere than ${AGENTS_LINK_TARGET}. Use --force to replace the link.`
        : `${agents.link} exists and is not a link to ${AGENTS_LINK_TARGET}. Remove it or install without --agents-link.`
    )
  await mkdir(skillsDir, { recursive: true })

  for (const name of await skillNames(kitRoot)) {
    const destination = path.join(skillsDir, name)
    const manifestPath = path.join(destination, MANIFEST)
    if (await exists(manifestPath)) {
      const current = JSON.parse(await readFile(manifestPath, 'utf8'))
      const shipped = await hashDirectory(path.join(kitRoot, 'skills', name))
      if (
        current.library === library.name &&
        current.version === library.version &&
        Boolean(current.agentsLink) === Boolean(linkAgents) &&
        current.hash === shipped &&
        (await hashDirectory(destination)) === shipped
      ) {
        unchanged.push(name)
        continue
      }
    }
    if (await exists(destination)) {
      const manifestFile = path.join(destination, MANIFEST)
      if (!(await exists(manifestFile))) {
        if (!force) {
          throw new Error(
            `${destination} exists and was not installed by this kit. Use --force to replace it.`
          )
        }
      } else {
        const current = JSON.parse(await readFile(manifestFile, 'utf8'))
        if (!force && compareVersions(current.version, library.version) > 0) {
          throw new Error(
            `${name} ${current.version} is newer than the kit in ${library.name}@${library.version}. Use --force to downgrade.`
          )
        }
      }
      await rm(destination, { recursive: true, force: true })
    }
    await cp(path.join(kitRoot, 'skills', name), destination, { recursive: true })
    await writeFile(
      path.join(destination, MANIFEST),
      `${JSON.stringify({ library: library.name, version: library.version, hash: await hashDirectory(destination), ...(linkAgents ? { agentsLink: true } : {}) }, null, 2)}\n`
    )
    installed.push(name)
  }

  let agentsLink = 'skipped'
  if (agents?.state === 'linked') agentsLink = 'already-linked'
  else if (agents) {
    if (agents.state === 'conflict') await rm(agents.link)
    await mkdir(path.dirname(agents.link), { recursive: true })
    await symlink(AGENTS_LINK_TARGET, agents.link, 'dir')
    agentsLink = agents.state === 'conflict' ? 'replaced' : 'created'
  }
  return { library, installed, unchanged, agentsLink }
}

// Reports why the installed skills are not what this library version ships. Used in a consumer's CI.
async function checkKit({ target, kitRoot = defaultKitRoot }) {
  const library = await readLibrary(kitRoot)
  const problems = []
  for (const name of await skillNames(kitRoot)) {
    const destination = path.join(target, '.claude', 'skills', name)
    const manifestFile = path.join(destination, MANIFEST)
    if (!(await exists(manifestFile))) {
      problems.push({
        skill: name,
        reason: 'not-installed',
        message: `${name} is not installed by the kit`
      })
      continue
    }
    const recorded = JSON.parse(await readFile(manifestFile, 'utf8'))
    if (recorded.version !== library.version) {
      problems.push({
        skill: name,
        reason: 'stale',
        message: `${name} is ${recorded.version}, ${library.name}@${library.version} ships a different version. Run install.`
      })
    } else if ((await hashDirectory(destination)) !== recorded.hash) {
      problems.push({
        skill: name,
        reason: 'modified',
        message: `${name} was edited after install`
      })
    } else if ((await hashDirectory(path.join(kitRoot, 'skills', name))) !== recorded.hash) {
      problems.push({
        skill: name,
        reason: 'stale',
        message: `${name} differs from the kit at the same version`
      })
    }
  }
  // The link is checked only when an install asked for it (recorded in the manifest), so a consumer's own
  // .agents/skills directory is never a problem.
  const manifests = await Promise.all(
    (await skillNames(kitRoot)).map((name) =>
      readFile(path.join(target, '.claude', 'skills', name, MANIFEST), 'utf8')
        .then((text) => JSON.parse(text))
        .catch(() => ({}))
    )
  )
  const agents = await agentsLinkState(target)
  if (manifests.some((manifest) => manifest.agentsLink === true) && agents.state !== 'linked')
    problems.push({
      skill: '.agents/skills',
      reason: 'agents-link-mismatch',
      message: `${agents.link} was installed with --agents-link but is not a link to ${AGENTS_LINK_TARGET}`
    })
  return { library, ok: problems.length === 0, problems }
}

export { checkKit, installKit }
