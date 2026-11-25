// The HUD section's pages: the Explorer HUD's Showcase as a whole, each of its sections, then its glyphs. The section
// index (apps/hud/design/ShowcaseIndex.ts) names them without loading the HUD, which only the HUD canvas (hud.html)
// loads.
import { SHOWCASE_INDEX, type ShowcaseEntry } from '../../apps/hud/design/ShowcaseIndex'
import { pageAliases, type HudPages } from './url'

/** The page with every section in one window, as `?showcase=1` shows them in a HUD host, then the glyphs. */
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

export const hudSections: readonly ShowcaseEntry[] = [...SHOWCASE_INDEX, HUD_GLYPHS]

/** The `hud` pages, the overview first, and the section each other component opens (TextArea shows FormControls). */
export const hudPages: HudPages = {
  pages: [HUD_OVERVIEW, ...hudSections.map((section) => section.name)],
  aliases: pageAliases(hudSections)
}
