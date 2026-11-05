import type { RawComponentPage } from '../ir/raw-components.js'
import type { LiveNode } from './live-file.js'
import { componentNodes } from './rest-capture.js'

function assertCompleteCapture(
  previous: RawComponentPage | undefined,
  current: LiveNode,
  retired: (componentId: string) => boolean = () => false
): void {
  if (!previous) return
  if (previous.page.id !== current.id)
    throw new Error(`Capture page identity changed: ${current.id}`)
  const components = new Map(componentNodes(current).map(({ node }) => [node.id, node]))
  for (const component of previous.components) {
    if (retired(component.id)) continue
    const live = components.get(component.id)
    if (!live)
      throw new Error(`Incomplete capture: missing component ${component.id} on page ${current.id}`)
    const variants = new Set(
      live.type === 'COMPONENT_SET'
        ? (live.children ?? [])
            .filter((node) => node.type === 'COMPONENT' && node.visible !== false)
            .map((node) => node.id)
        : [live.id]
    )
    for (const variant of [component.base, ...component.others])
      if (!variants.has(variant.id))
        throw new Error(`Incomplete capture: missing variant ${variant.id} on page ${current.id}`)
  }
}

export { assertCompleteCapture }
