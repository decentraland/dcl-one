import path from 'node:path'
import { loadConfig } from '../src/config.js'
import type { ComponentSpec } from '../src/ir/components.js'
import { SnapshotSource } from '../src/sources/snapshot-source.js'
import { loadSpecsFromSource } from '../src/specs.js'

const projectRoot = path.resolve(import.meta.dirname, '..')
// Tests read a frozen copy so a live Figma change cannot break them (scripts/freeze-test-snapshot.mjs).
const snapshotDir = path.resolve(projectRoot, 'test/fixtures/figma-snapshot')

const snapshotSource = () => new SnapshotSource(snapshotDir)

const FIXTURE_ALIASES = { Button: 'ButtonSet' }

let cached: Promise<ComponentSpec[]> | undefined

// Building the specs expands ~170 variants, so every test file shares one result per worker.
function componentSpecs(): Promise<ComponentSpec[]> {
  cached ??= (async () => {
    const base = await loadConfig(projectRoot)
    const config = {
      ...base,
      selection: undefined,
      // The frozen capture holds every page, so the set named "Button" collides with the aliased
      // Button-Contained-Text; a test-only alias keeps both. Same-named sets stay excluded.
      componentNames: { ...base.componentNames, ...FIXTURE_ALIASES }
    }
    const source = snapshotSource()
    // Mapper tests cover every capture, independently of publication inventory.
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

export { componentSpec, componentSpecs, projectRoot, snapshotDir, snapshotSource }
