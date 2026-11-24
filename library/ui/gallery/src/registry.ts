import type { ReactNode } from 'react'

/** One rendering of a component: a variant, a state or a composition, with deterministic data. */
export type Example = {
  id: string
  title: string
  /** What the example demonstrates when the title alone does not say it. */
  note?: string
  /** `light` draws the example on a light surface, for controls made for one. */
  surface?: 'light'
  render: () => ReactNode
}

/** A gallery page: the public exports it renders and their examples. */
export type GalleryEntry = {
  /** The page name, also its `component` URL parameter. */
  name: string
  group: string
  summary: string
  /** Every public component export the examples render; the coverage gate reads this list. */
  components: readonly string[]
  examples: readonly Example[]
  /** A taller canvas, for content positioned against the viewport (dialogs, viewport toasts). */
  minHeight?: number
}

/** A public component export the gallery does not show, with the reason. */
export type Exclusion = { name: string; reason: string }
