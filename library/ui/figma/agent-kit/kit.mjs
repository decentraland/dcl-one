import { createHash } from 'node:crypto'
import { cp, lstat, mkdir, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const defaultKitRoot = path.dirname(fileURLToPath(import.meta.url))
const MANIFEST = '.kit.json'

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

async function skillNames(kitRoot) {
  const entries = await readdir(path.join(kitRoot, 'skills'), { withFileTypes: true })
  return entries.filter((entry) => entry.isDirectory()).map((entry) => entry.name)
}

// Copies the skills of this library version into <target>/.claude/skills. A skill directory the kit did
// not create (no manifest) belongs to the consumer and is never overwritten without --force.
async function installKit({ target, force = false, linkAgents = true, kitRoot = defaultKitRoot }) {
  const library = await readLibrary(kitRoot)
  const skillsDir = path.join(target, '.claude', 'skills')
  const installed = []
  await mkdir(skillsDir, { recursive: true })

  for (const name of await skillNames(kitRoot)) {
    const destination = path.join(skillsDir, name)
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
      `${JSON.stringify({ library: library.name, version: library.version, hash: await hashDirectory(destination) }, null, 2)}\n`
    )
    installed.push(name)
  }

  let agentsLink = 'skipped'
  if (linkAgents) {
    const agentsDir = path.join(target, '.agents')
    const link = path.join(agentsDir, 'skills')
    if (await exists(link)) {
      agentsLink = 'already-present'
    } else {
      await mkdir(agentsDir, { recursive: true })
      await symlink('../.claude/skills', link, 'dir')
      agentsLink = 'created'
    }
  }
  return { library, installed, agentsLink }
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
  return { library, ok: problems.length === 0, problems }
}

export { checkKit, installKit }
