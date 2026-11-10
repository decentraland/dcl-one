import { assertIdentifier } from '../generators/naming.js'
import type { ComponentSpec, TextRole, TextRoleSpec, ViewNode } from '../ir/components.js'

// A role name, and a role's href, become bindings in the generated component's parameter destructure, next to the
// variant, slot and semantic props. They must be camelCase identifiers that the shared generator validation accepts
// (no reserved words, no strict-mode names such as eval or arguments), and must not shadow what the generated module
// binds itself.
const PROP_NAME = /^[a-z][A-Za-z0-9]*$/
function isPropName(name: string): boolean {
  if (!PROP_NAME.test(name)) return false
  try {
    assertIdentifier(name)
    return true
  } catch {
    return false
  }
}
const generatedBindings = (spec: ComponentSpec) => {
  const base = spec.name.charAt(0).toLowerCase() + spec.name.slice(1)
  return [
    'rest',
    'forceState',
    'styles',
    'FigmaView',
    'registerComponent',
    `${base}Views`,
    `${base}Cases`,
    `${base}SuppressedProps`
  ]
}

type Located = { node: ViewNode; namePath: string[] }

function locate(spec: ComponentSpec): Map<string, Located> {
  const byId = new Map<string, Located>()
  const visit = (node: ViewNode, names: string[]) => {
    const namePath = [...names, node.name]
    if (node.figmaId) {
      if (byId.has(node.figmaId))
        throw new Error(
          `${spec.name}: Figma node id ${node.figmaId} appears more than once in the capture`
        )
      byId.set(node.figmaId, { node, namePath })
    }
    node.children?.forEach((child) => visit(child, namePath))
  }
  spec.views.forEach((view) => visit(view.root, []))
  return byId
}

// Binds each configured role to its captured text nodes by Figma node id, renaming their slot to the role. Any id the
// capture lacks, any node whose layer names differ from the configured path, any node that is not text and any node
// claimed twice fails generation: a role never falls back to a positional slot.
function applyTextRoles(spec: ComponentSpec, roles: Record<string, TextRole>): TextRoleSpec[] {
  const fail = (message: string): never => {
    throw new Error(`${spec.name} text roles: ${message}`)
  }
  const reserved = new Set([
    ...spec.props.map((prop) => prop.name),
    ...spec.slots.map((slot) => slot.name),
    ...generatedBindings(spec)
  ])
  for (const [name, role] of Object.entries(roles)) {
    if (!isPropName(name)) fail(`${name} is not a valid prop name`)
    if (reserved.has(name))
      fail(`${name} collides with an existing prop, slot or generated binding`)
    if (role.type === 'count') {
      if (!role.label) fail(`count role ${name} needs a label role`)
      if (roles[role.label!]?.type !== 'string')
        fail(`count role ${name} names ${role.label}, which is not a string role`)
    } else if (role.label) fail(`string role ${name} cannot have a label`)
    if (role.href !== undefined) {
      if (!isPropName(role.href)) fail(`${name}: href ${role.href} is not a valid prop name`)
      if (reserved.has(role.href) || role.href in roles)
        fail(
          `${name}: href ${role.href} collides with an existing prop, slot, role or generated binding`
        )
      const owner = Object.entries(roles).find(
        ([other, item]) => other !== name && item.href === role.href
      )
      if (owner) fail(`${name} and ${owner[0]} declare the same href ${role.href}`)
    }
  }
  const labels = new Set(Object.values(roles).flatMap((role) => (role.label ? [role.label] : [])))

  const byId = locate(spec)
  const claimed = new Map<string, string>()
  const bindings: [ViewNode, string][] = []
  for (const [name, role] of Object.entries(roles)) {
    // A label role is the consumer's text for an accessible name; it binds no captured text of its own.
    if (labels.has(name)) {
      if (role.nodes.length) fail(`label role ${name} must not bind captured nodes`)
      continue
    }
    if (!role.nodes.length) fail(`${name} binds no captured text node`)
    for (const { nodeId, namePath } of role.nodes) {
      const found = byId.get(nodeId)
      if (!found) fail(`${name}: unknown Figma node id ${nodeId}`)
      const { node, namePath: actual } = found!
      if (JSON.stringify(actual) !== JSON.stringify(namePath))
        fail(
          `${name}: name path mismatch for ${nodeId}: expected ${JSON.stringify(namePath)}, found ${JSON.stringify(actual)}`
        )
      if (node.kind !== 'text') fail(`${name}: node ${nodeId} is not a text layer`)
      const owner = claimed.get(nodeId)
      if (owner) fail(`${name}: node ${nodeId} is already bound to ${owner}`)
      claimed.set(nodeId, name)
      bindings.push([node, name])
    }
  }
  // Every check passed: only now are the captured nodes renamed, so a failure leaves the views untouched.
  for (const [node, name] of bindings) node.slot = name
  return Object.entries(roles).map(([name, role]) => ({
    name,
    type: role.type,
    ...(role.label ? { label: role.label } : {}),
    ...(role.href ? { href: role.href } : {})
  }))
}

export { applyTextRoles }
