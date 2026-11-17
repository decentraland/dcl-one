import type { ComponentSpec } from '../ir/components.js'

// The promotion seal, its verification and promote-reviewed belong to figma-design-system, where the capture they
// hash lives. This tree keeps the eligibility rule check-mappings uses to report a synced replacement's blockers,
// and the runtime files a generated component needs.

/** The Review runtime files every generated component imports, relative to figma/review/src/components. */
const runtimeFiles = [
  'figma-module-runtime.tsx',
  'figma-module-runtime.module.css',
  'figma-view-state.ts',
  'figma-interactions.tsx'
]

// Promotion skips components with unresolved needs-manual items and the Progress Bar pending visual review.
const promotionExclusion = (spec: ComponentSpec): string[] =>
  spec.page === 'Progress Bar' ? ['Pending visual review'] : spec.needsManual

export { promotionExclusion, runtimeFiles }
