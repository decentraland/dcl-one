/**
 * Wrap a base `Composite.Provider` so every composite it returns has its
 * `{assetPath}` placeholders resolved in place before being handed to the engine.
 *
 * This `{assetPath}` substitution is the only composite-resolution behavior
 * smart items add. Smart items don't bundle composites themselves today (they
 * ship as on-disk asset directories the SDK provider serves), and their component
 * types are registered directly via `createComponents(engine)` at init — not
 * through a provider `schemas` array — so there's no separate smart-item provider
 * to compose in. The scene entrypoint simply wraps the SDK provider with this and
 * re-registers it.
 *
 * Asset-pack composite JSON on disk stores texture / GLB / audio paths as
 * `{assetPath}/relative/path.ext` so they're portable across asset-dir layouts;
 * substitution rewrites them to the resolved paths the engine expects.
 *
 * Mutates the cached resource in place: the base provider keeps a single resource
 * by reference and we mutate it on first resolution. Substitution is idempotent,
 * so a re-resolution after a second `getCompositeOrNull(src)` is a no-op.
 *
 * Composites authored before the runtime became @dcl/smart-items still name their
 * components under the legacy prefix; they are renamed in place the same way.
 */

import type { Composite } from '@dcl/ecs'
import { substituteAssetPathInComposite } from './add-child'
import { renameLegacyComposite } from './legacy'

export function wrapWithAssetPathSubstitution(base: Composite.Provider): Composite.Provider {
  return {
    schemas: base.schemas,
    getCompositeOrNull(src: string) {
      const resource = base.getCompositeOrNull(src)
      if (resource) substituteAssetPathInComposite(renameLegacyComposite(resource.composite), src)
      return resource
    },
    loadComposite: base.loadComposite
      ? async (src: string) => {
          const resource = await base.loadComposite!(src)
          substituteAssetPathInComposite(renameLegacyComposite(resource.composite), src)
          return resource
        }
      : undefined
  }
}
