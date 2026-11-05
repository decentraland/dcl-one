import type { Ownership } from './ownership.js'

type ComponentOwner = { pageId: string; componentId: string }
function componentOwnerById(ownership: Ownership, componentId: string): ComponentOwner | null {
  const pageId = ownership.componentPageById.get(componentId)
  return pageId === undefined ? null : { pageId, componentId }
}
function componentOwner(ownership: Ownership, name: string | undefined): ComponentOwner | null {
  if (name === undefined) return null
  const id = ownership.componentIdByName.get(name)
  if (id) return componentOwnerById(ownership, id)
  return null
}
export { componentOwner, componentOwnerById }
export type { ComponentOwner }
