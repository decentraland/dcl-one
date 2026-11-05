import { createHash } from 'node:crypto'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { parse } from 'yaml'

// Only packages that reach the rendered page belong in the signature: the Review runtime dependencies
// and the bundler. Hashing the whole lockfile made every lint or type-checking update invalidate the
// visual evidence.
const RENDER_PACKAGES = ['@fontsource/inter', '@tanstack/react-store', 'react', 'react-dom', 'vite']

// The render dependencies come from the lockfile, not from node_modules: resolved versions and
// integrity of every package reachable from RENDER_PACKAGES, including each platform variant of a
// native binding (rolldown, lightningcss, fsevents) the lock lists. The signature is therefore the
// same on macOS and Linux for the same lockfile, while any dependency bump still changes it.
async function renderDependencies(packageRoot, names = RENDER_PACKAGES) {
  const lock = parse(await readFile(path.join(packageRoot, 'pnpm-lock.yaml'), 'utf8'))
  const importer = lock.importers?.['.'] ?? {}
  const direct = {
    ...importer.dependencies,
    ...importer.devDependencies,
    ...importer.optionalDependencies
  }
  const visited = new Set()
  const pending = names.map((name) => {
    if (!direct[name]) throw new Error(`${name} is not a dependency in pnpm-lock.yaml`)
    return [name, direct[name].version]
  })
  while (pending.length) {
    const [name, version] = pending.pop()
    const snapshotKey = `${name}@${version}`
    if (visited.has(snapshotKey)) continue
    visited.add(snapshotKey)
    const manifest = lock.packages?.[packageKey(name, version)]
    if (!manifest) throw new Error(`${packageKey(name, version)} is missing from pnpm-lock.yaml`)
    const snapshot = lock.snapshots?.[snapshotKey] ?? {}
    const optionalPeers = manifest.peerDependenciesMeta ?? {}
    for (const field of ['dependencies', 'optionalDependencies'])
      for (const [dependency, resolved] of Object.entries(snapshot[field] ?? {}))
        // Optional peers such as @types/node, tsx or sass are integrations the page does not use.
        if (!optionalPeers[dependency]?.optional) pending.push([dependency, resolved])
  }
  return [
    ...new Set(
      [...visited].map((key) => {
        const at = key.lastIndexOf('@', key.indexOf('(') === -1 ? undefined : key.indexOf('('))
        const base = packageKey(key.slice(0, at), key.slice(at + 1))
        return `${base} ${lock.packages[base].resolution?.integrity ?? ''}`
      })
    )
  ]
    .sort()
    .join('\n')
}

// The packages section is keyed by name@version without the peer suffix snapshots carry.
const packageKey = (name, version) => `${name}@${String(version).split('(')[0]}`

const UNRENDERED = new Set([
  'src/review/promotion.json',
  'src/review/InteractivePreview.tsx',
  'src/review/InteractivePreview.module.css',
  'src/review/InteractivePreview.helpers.ts'
])

async function reviewSignature(root) {
  const files = [
    'src/components/StateMatrix.tsx',
    'src/components/StateMatrix.module.css',
    'src/components/figma-module-runtime.tsx',
    'src/components/figma-module-runtime.module.css',
    'src/components/figma-view-state.ts',
    'src/components/figma-interactions.tsx'
  ]
  async function collect(directory) {
    for (const entry of await readdir(path.join(root, directory), { withFileTypes: true })) {
      if (
        /^\.figma-tmp-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          entry.name
        )
      )
        continue
      const name = directory + '/' + entry.name
      if (entry.isDirectory()) await collect(name)
      // Approval metadata is generated after capture, and the playground is never rendered in audit
      // mode (main.tsx returns the bare stage first), so neither alters the captured output.
      else if (!UNRENDERED.has(name)) files.push(name)
    }
  }
  await collect('src/review')
  await collect('src/generated')
  await collect('public/dcl-ui-review/references')
  const hash = createHash('sha256')
    .update(await renderDependencies(path.resolve(root, '../..')))
    .update('\0')
  for (const file of files.sort())
    hash
      .update(file)
      .update('\0')
      .update(await readFile(path.join(root, file)))
      .update('\0')
  return hash.digest('hex')
}
export { renderDependencies, reviewSignature }
