import { cmd } from '../cmd'
import { state } from '../state'
import { decodeCustomComponents, isCustomComponent, NAME_COMPONENT } from '../custom-components'
import { buildComposite } from '../composite'
import { computeSaveDiff, buildAuthoredFromSelection, defaultSelection, type DiffSource } from '../save-diff'
import { getSchema, loadSchema, toSdkValue } from '../schema'
import { loadComponentNames } from './component-ops'


export async function exportComposite(): Promise<string> {
  if (state.componentNames.length === 0) await loadComponentNames()
  const initial = await cmd.crdtInitial()
  decodeCustomComponents(initial)
  const rows = computeSaveDiff(initial, state.snapshot)
  const selection = new Map<string, DiffSource>()
  for (const row of rows) {
    selection.set(`${row.entityId}/${row.component}`, defaultSelection(row))
  }
  const authored = buildAuthoredFromSelection(initial, rows, selection)
  if (state.scene?.hash?.startsWith('b64-')) {
    for (const [eid, comps] of Object.entries(authored)) {
      if (eid !== '0' && comps[NAME_COMPONENT] === undefined) delete authored[eid]
    }
  }

  const protoNames = new Set<string>()
  for (const comps of Object.values(authored)) {
    for (const name of Object.keys(comps)) {
      if (!isCustomComponent(name)) protoNames.add(name)
    }
  }
  await Promise.all([...protoNames].map(loadSchema))
  for (const comps of Object.values(authored)) {
    for (const name of Object.keys(comps)) {
      if (isCustomComponent(name)) continue
      const schema = getSchema(name)
      if (schema !== undefined) comps[name] = toSdkValue(comps[name], schema.root)
    }
  }

  return buildComposite(authored)
}
