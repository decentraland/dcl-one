import { createHash } from 'node:crypto'
import { cp, readdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { loadConfigFile, type FigmaConfig } from '../src/config.js'
import type { ComponentSpec } from '../src/ir/components.js'
import { digestTableHash, MARKER } from '../src/sources/fds-source.js'
import { SnapshotSource } from '../src/sources/snapshot-source.js'
import { loadSpecsFromSource } from '../src/specs.js'

const projectRoot = path.resolve(import.meta.dirname, '..')
// Tests read a hand-written synthetic snapshot (fixtures/synthetic-snapshot/README.md), never a Figma capture,
// with the fixture config that describes its pages.
const fixturesDir = path.resolve(projectRoot, 'test/fixtures')
const snapshotDir = path.join(fixturesDir, 'synthetic-snapshot')
const testConfigFile = path.join(fixturesDir, 'figma.config.json')

const snapshotSource = () => new SnapshotSource(snapshotDir)

const testConfig = (): Promise<FigmaConfig> => loadConfigFile(testConfigFile)

// Pins whatever capture a test tree holds, the way fetch-source does: a lock with its digest table next to the
// config and the marker in the capture, so a CLI or MCP subprocess reads it through the verified boundary.
async function pinTestCapture(figmaDir: string, configName = 'figma.config.json'): Promise<void> {
  const configFile = path.join(figmaDir, configName)
  const config = JSON.parse(await readFile(configFile, 'utf8'))
  const capture = path.resolve(figmaDir, config.snapshotDir)
  const digests: Record<string, string> = {}
  const visit = async (relative: string): Promise<void> => {
    for (const entry of await readdir(path.join(capture, relative), { withFileTypes: true })) {
      const file = relative ? `${relative}/${entry.name}` : entry.name
      if (entry.isDirectory()) await visit(file)
      else if (file !== MARKER)
        digests[file] = createHash('sha256')
          .update(await readFile(path.join(capture, file)))
          .digest('hex')
    }
  }
  await visit('')
  const revision = 'c'.repeat(40)
  await writeFile(
    path.join(figmaDir, 'test-capture.lock.json'),
    JSON.stringify({
      sources: [
        {
          repository: 'https://github.com/decentraland/figma-design-system',
          revision,
          snapshot: { revision, digests }
        }
      ]
    })
  )
  await writeFile(
    path.join(capture, MARKER),
    JSON.stringify({ revision, digests: digestTableHash(digests) })
  )
  await writeFile(configFile, JSON.stringify({ ...config, snapshotLock: 'test-capture.lock.json' }))
}

// Copies the snapshot data into a test tree as a capture directory, without the fixture's README, which no
// capture holds and the ownership guard would rightly report as unowned.
const copySnapshot = (target: string) =>
  cp(snapshotDir, target, {
    recursive: true,
    filter: (file) => file !== path.join(snapshotDir, 'README.md')
  })

let cached: Promise<ComponentSpec[]> | undefined

// Every test file shares one build of the specs per worker.
function componentSpecs(): Promise<ComponentSpec[]> {
  cached ??= (async () => {
    const config = { ...(await testConfig()), selection: undefined }
    const source = snapshotSource()
    // Mapper tests cover every page of the snapshot, independently of publication inventory.
    source.getGenerationInventory = async () => undefined
    return (await loadSpecsFromSource(source, config)).specs
  })()
  return cached
}

async function componentSpec(name: string): Promise<ComponentSpec> {
  const spec = (await componentSpecs()).find((entry) => entry.name === name)
  if (!spec) throw new Error(`Spec ${name} not found in the snapshot`)
  return spec
}

export {
  copySnapshot,
  componentSpec,
  componentSpecs,
  fixturesDir,
  pinTestCapture,
  projectRoot,
  snapshotDir,
  snapshotSource,
  testConfig,
  testConfigFile
}
