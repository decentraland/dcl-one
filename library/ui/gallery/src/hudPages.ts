// The HUD section's pages: the Explorer HUD's Showcase as a whole, then each of its sections and its glyphs, by name.
// The section index (apps/hud/design/ShowcaseIndex.ts) names them without loading the HUD, which only the HUD canvas
// (hud.html) loads.
import { SHOWCASE_INDEX, type ShowcaseEntry } from '../../apps/hud/design/ShowcaseIndex'
import { sortByLabel } from './order'
import { pageAliases, type HudPages } from './url'

/** The page with every section and the glyphs in one window, in the sidebar's order. */
export const HUD_OVERVIEW = 'Showcase'

/**
 * The HUD's icon set (design/Glyphs.tsx), every glyph drawn by the gallery itself, as it draws the foundations'
 * icons: a Showcase section would keep the glyphs the HUD does not use in its design chunk.
 */
export const HUD_GLYPHS = {
  name: 'Glyphs',
  title: 'Glyphs (every icon design/Glyphs.tsx exports, by name)',
  components: []
} satisfies ShowcaseEntry

const declared: readonly ShowcaseEntry[] = [...SHOWCASE_INDEX, HUD_GLYPHS]

/** The sections and the glyphs by name (`sortByLabel`): the order of the sidebar and of the Showcase page. */
export const hudSections: readonly ShowcaseEntry[] = sortByLabel(
  declared,
  (section) => section.name
)

/**
 * The `hud` pages, the overview first, and the section each other component opens (TextArea shows FormControls): of
 * the sections that render it, the first in the HUD's own index.
 */
export const hudPages: HudPages = {
  pages: [HUD_OVERVIEW, ...hudSections.map((section) => section.name)],
  aliases: pageAliases(declared)
}
