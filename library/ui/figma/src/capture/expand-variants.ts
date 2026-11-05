import type { RawComponent, RawNode, RawPatch } from '../ir/raw-components.js'

type ExpandedVariant = { id: string; name: string; props: Record<string, string>; tree: RawNode }

function nodeAt(root: RawNode, path: number[]): RawNode {
  let node = root
  for (const index of path) {
    const child = node.children?.[index]
    if (!child) throw new Error(`Patch path [${path.join(',')}] does not exist`)
    node = child
  }
  return node
}

function applyPatch(root: RawNode, patch: RawPatch): RawNode {
  if (patch.replace) {
    if (patch.at.length === 0) return structuredClone(patch.replace)
    const parent = nodeAt(root, patch.at.slice(0, -1))
    const index = patch.at[patch.at.length - 1]!
    if (!parent.children?.[index])
      throw new Error(`Patch path [${patch.at.join(',')}] does not exist`)
    parent.children[index] = structuredClone(patch.replace)
    return root
  }
  const target = nodeAt(root, patch.at) as Record<string, unknown>
  for (const key of patch.unset ?? []) delete target[key]
  Object.assign(target, structuredClone(patch.set ?? {}))
  return root
}

function expandVariants(component: RawComponent): ExpandedVariant[] {
  const { base, others } = component
  return [
    { id: base.id, name: base.name, props: base.props, tree: structuredClone(base.tree) },
    ...others.map((variant) => ({
      id: variant.id,
      name: variant.name,
      props: variant.props,
      tree: variant.patches.reduce(applyPatch, structuredClone(base.tree))
    }))
  ]
}

export { applyPatch, expandVariants }
export type { ExpandedVariant }
