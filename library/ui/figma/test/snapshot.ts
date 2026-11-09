import { cp } from 'node:fs/promises'
import path from 'node:path'
import { loadConfigFile, type FigmaConfig } from '../src/config.js'
import type { ComponentSpec } from '../src/ir/components.js'
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
  projectRoot,
  snapshotDir,
  snapshotSource,
  testConfig,
  testConfigFile
}
