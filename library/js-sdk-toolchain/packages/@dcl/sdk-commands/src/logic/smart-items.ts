/** The component namespace composites carried before the smart-item runtime moved into the SDK. */
export const LEGACY_PREFIX = 'asset-packs::'
/** The component namespace of every smart-item component. */
export const PREFIX = 'smart-items::'
/** The build-time component listing the scripts attached to an entity. */
export const SCRIPT_COMPONENT = 'smart-items::Script'

/** Whether a component name belongs to the smart-item runtime, under either prefix. */
export function isEditorComponent(name: string): boolean {
  return name.startsWith(PREFIX) || name.startsWith(LEGACY_PREFIX)
}

/** Swaps the legacy prefix for the current one; names already renamed are returned as they are. */
export function renameLegacyComponentName(name: string): string {
  return name.startsWith(LEGACY_PREFIX) ? PREFIX + name.slice(LEGACY_PREFIX.length) : name
}

/** Renames every legacy component name of a composite in place and returns the composite. */
export function renameLegacyComposite<T extends { components?: { name: string }[] }>(json: T): T {
  for (const component of json.components ?? []) {
    if (typeof component.name === 'string') component.name = renameLegacyComponentName(component.name)
  }
  return json
}
