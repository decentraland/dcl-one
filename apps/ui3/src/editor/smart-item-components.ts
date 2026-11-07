// The composite component namespace of the smart-item runtime (`@dcl/sdk/smart-items`):
// Actions, Triggers, States, Counter and the rest are `smart-items::<Name>`, and the
// component number the engine derives from each name follows the namespace.
export const SMART_ITEM_PREFIX = "smart-items::";

// Composites saved by the Scene Editor can name the same components `asset-packs::<Name>`.
// Every reader of a composite (Sites' parsers and the editor-scene's hydration) renames that
// prefix on load, so such scenes open, build and publish unchanged; everything written back
// carries `smart-items::`.
export const LEGACY_SMART_ITEM_PREFIX = "asset-packs::";

/** The current name of a composite component: `asset-packs::X` reads as `smart-items::X`. */
export function smartItemComponentName(name: string): string {
  return name.startsWith(LEGACY_SMART_ITEM_PREFIX)
    ? SMART_ITEM_PREFIX + name.slice(LEGACY_SMART_ITEM_PREFIX.length)
    : name;
}

/**
 * The component blocks of a composite with every legacy smart-item name renamed. Blocks that
 * are not objects with a string `name` pass through untouched, as do blocks already named
 * under the current namespace, so the result is safe on unvalidated wire JSON.
 */
export function renameLegacySmartItemComponents<T>(components: readonly T[]): T[] {
  return components.map((block) => {
    if (typeof block !== "object" || block === null) return block;
    const name = (block as { name?: unknown }).name;
    if (typeof name !== "string") return block;
    const renamed = smartItemComponentName(name);
    return renamed === name ? block : ({ ...(block as object), name: renamed } as T);
  });
}
