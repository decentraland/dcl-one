/** The component namespace composites carried before the runtime moved into the SDK. */
export const LEGACY_PREFIX = 'asset-packs::'
/** The component namespace of every smart-item component. */
export const PREFIX = 'smart-items::'

/** Swaps the legacy prefix for the current one; names already renamed are returned as they are. */
export function renameLegacyComponentName(name: string): string {
  return name.startsWith(LEGACY_PREFIX) ? PREFIX + name.slice(LEGACY_PREFIX.length) : name
}

/** Renames every legacy component name of a composite in place and returns the composite. */
export function renameLegacyComposite<T extends { components?: { name: string }[] }>(json: T): T {
  for (const component of json.components ?? []) {
    component.name = renameLegacyComponentName(component.name)
  }
  return json
}
