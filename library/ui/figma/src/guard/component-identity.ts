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
function componentIdOfFile(ownership: Ownership, file: string): string | undefined {
  const prefix = file.startsWith('components/')
    ? 'components/'
    : 'figma/review/src/generated/components/'
  if (!file.startsWith(prefix)) return undefined
  const relative = file.slice(prefix.length)
  const suffixes = ['.tsx', '.module.css', '.module.css.d.ts', '.figma.ts']
  for (const suffix of suffixes) {
    if (!relative.endsWith(suffix)) continue
    const id = ownership.fileComponentId.get(relative.slice(0, -suffix.length))
    if (id) return id
  }
  return undefined
}

export { componentOwner, componentOwnerById, componentIdOfFile }
export type { ComponentOwner }
