import { Buffer } from 'node:buffer'
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

// Relative paths with / separators, so the listing and the hash are the same on Windows.
async function listFiles(dir, base = dir) {
  const files = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) files.push(...(await listFiles(full, base)))
    else if (entry.name !== MANIFEST)
      files.push(path.relative(base, full).split(path.sep).join('/'))
  }
  return files.sort()
}

// Skills are text. CRLF is hashed as LF, so a checkout with core.autocrlf=true matches the manifest; each file
// is framed by its name, a NUL and its length, so bytes cannot move between a name and its content.
async function hashDirectory(dir) {
  const hash = createHash('sha256')
  for (const file of await listFiles(dir)) {
    const content = Buffer.from(
      (await readFile(path.join(dir, file), 'latin1')).replaceAll('\r\n', '\n'),
      'latin1'
    )
    hash.update(`${file}\0${content.length}\0`)
    hash.update(content)
  }
  return hash.digest('hex')
}

// The manifest, or { invalid } when it exists but is not a kit manifest; undefined when there is none.
async function readManifest(dir) {
  const file = path.join(dir, MANIFEST)
  let text
  try {
    text = await readFile(file, 'utf8')
  } catch (error) {
    // No manifest, or a skill path that is a file: either way the kit did not install it.
    if (error.code === 'ENOENT' || error.code === 'ENOTDIR') return undefined
    throw error
  }
  try {
    const manifest = JSON.parse(text)
    if (manifest && typeof manifest === 'object' && typeof manifest.version === 'string')
      return manifest
  } catch {
    // Not JSON: the caller reports the file by name.
  }
  return { invalid: `${file} is not a valid kit manifest` }
}

// .claude and .claude/skills must be real directories (or absent): a symlink would make mkdir, rm and cp write
// outside the target.
async function linkedSkillsParent(target) {
  for (const dir of [path.join(target, '.claude'), path.join(target, '.claude', 'skills')]) {
    const stat = await lstat(dir).catch((error) => {
      if (error.code === 'ENOENT') return undefined
      throw error
    })
    if (stat?.isSymbolicLink()) return dir
    if (stat && !stat.isDirectory()) return dir
  }
  return undefined
}

const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/

// Semantic version order (a prerelease sorts before its release); undefined when either is not a version, which
// the callers treat as a version they cannot order.
function compareVersions(a, b) {
  const pa = SEMVER.exec(String(a))
  const pb = SEMVER.exec(String(b))
  if (!pa || !pb) return undefined
  for (let i = 1; i <= 3; i += 1)
    if (Number(pa[i]) !== Number(pb[i])) return Number(pa[i]) - Number(pb[i])
  if (pa[4] === pb[4]) return 0
  if (pa[4] === undefined) return 1
  if (pb[4] === undefined) return -1
  const xa = pa[4].split('.')
  const xb = pb[4].split('.')
  for (let i = 0; i < Math.min(xa.length, xb.length); i += 1) {
    if (xa[i] === xb[i]) continue
    const na = /^\d+$/.test(xa[i])
    const nb = /^\d+$/.test(xb[i])
    if (na && nb) return Number(xa[i]) - Number(xb[i])
    if (na !== nb) return na ? -1 : 1
    return xa[i] < xb[i] ? -1 : 1
  }
  return xa.length - xb.length
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
// not create (no manifest) belongs to the consumer and is never overwritten without --force, nor is a newer, an
// unordered or an unreadable install. A skill that already matches this version byte for byte is left untouched,
// so a repeated install is a no-op. The .agents/skills link is opt-in; an existing entry that is not the kit's
// link is a conflict, and --force replaces a foreign symlink but never removes a real directory. Every refusal,
// a symlinked .claude or .claude/skills included, happens before anything is written.
async function installKit({ target, force = false, linkAgents = false, kitRoot = defaultKitRoot }) {
  const library = await readLibrary(kitRoot)
  const skillsDir = path.join(target, '.claude', 'skills')
  const linked = await linkedSkillsParent(target)
  if (linked)
    throw new Error(
      `${linked} is a link or not a directory. The kit writes only into a real directory.`
    )
  const agents = linkAgents ? await agentsLinkState(target) : undefined
  if (agents?.state === 'conflict' && (!force || !agents.symlink))
    throw new Error(
      agents.symlink
        ? `${agents.link} links elsewhere than ${AGENTS_LINK_TARGET}. Use --force to replace the link.`
        : `${agents.link} exists and is not a link to ${AGENTS_LINK_TARGET}. Remove it or install without --agents-link.`
    )

  const plan = []
  for (const name of await skillNames(kitRoot)) {
    const destination = path.join(skillsDir, name)
    const shipped = await hashDirectory(path.join(kitRoot, 'skills', name))
    const stat = await lstat(destination).catch((error) => {
      if (error.code === 'ENOENT') return undefined
      throw error
    })
    if (!stat) {
      plan.push({ name, destination, replace: false })
      continue
    }
    if (!stat.isDirectory()) {
      if (!force)
        throw new Error(
          `${destination} is not a directory the kit installed. Use --force to replace it.`
        )
      plan.push({ name, destination, replace: true })
      continue
    }
    const current = await readManifest(destination)
    if (
      current &&
      !current.invalid &&
      current.library === library.name &&
      current.version === library.version &&
      Boolean(current.agentsLink) === Boolean(linkAgents) &&
      current.hash === shipped &&
      (await hashDirectory(destination)) === shipped
    ) {
      plan.push({ name, unchanged: true })
      continue
    }
    if (!force) {
      if (!current)
        throw new Error(
          `${destination} exists and was not installed by this kit. Use --force to replace it.`
        )
      if (current.invalid) throw new Error(`${current.invalid}. Use --force to replace it.`)
      const order = compareVersions(current.version, library.version)
      if (order === undefined)
        throw new Error(
          `${name} ${current.version} cannot be ordered against ${library.name}@${library.version}. Use --force to replace it.`
        )
      if (order > 0)
        throw new Error(
          `${name} ${current.version} is newer than the kit in ${library.name}@${library.version}. Use --force to downgrade.`
        )
    }
    plan.push({ name, destination, replace: true })
  }

  const installed = []
  const unchanged = []
  await mkdir(skillsDir, { recursive: true })
  for (const step of plan) {
    if (step.unchanged) {
      unchanged.push(step.name)
      continue
    }
    if (step.replace) await rm(step.destination, { recursive: true, force: true })
    await cp(path.join(kitRoot, 'skills', step.name), step.destination, { recursive: true })
    await writeFile(
      path.join(step.destination, MANIFEST),
      `${JSON.stringify({ library: library.name, version: library.version, hash: await hashDirectory(step.destination), ...(linkAgents ? { agentsLink: true } : {}) }, null, 2)}\n`
    )
    installed.push(step.name)
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
  const linked = await linkedSkillsParent(target)
  if (linked)
    return {
      library,
      ok: false,
      problems: [
        {
          skill: '.claude/skills',
          reason: 'not-installed',
          message: `${linked} is a link or not a directory; the kit installs only into a real directory`
        }
      ]
    }
  const manifests = []
  for (const name of await skillNames(kitRoot)) {
    const destination = path.join(target, '.claude', 'skills', name)
    const recorded = await readManifest(destination)
    if (!recorded) {
      problems.push({
        skill: name,
        reason: 'not-installed',
        message: `${name} is not installed by the kit`
      })
      continue
    }
    if (recorded.invalid) {
      problems.push({
        skill: name,
        reason: 'modified',
        message: `${recorded.invalid}. Run install --force.`
      })
      continue
    }
    manifests.push(recorded)
    if (recorded.version !== library.version) {
      // install refuses a newer or unordered version without --force, so check names the command that works.
      const order = compareVersions(recorded.version, library.version)
      problems.push({
        skill: name,
        reason: 'stale',
        message:
          order !== undefined && order < 0
            ? `${name} is ${recorded.version}, ${library.name}@${library.version} ships a newer version. Run install.`
            : `${name} is ${recorded.version}, newer than or not comparable with ${library.name}@${library.version}. Upgrade the library, or run install --force to downgrade.`
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
