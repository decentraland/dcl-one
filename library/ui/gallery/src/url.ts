// The gallery's URL contract. The runtime source reads `component`, `foundations` or `hud` (a section of the Explorer
// HUD's Showcase) and `width`; the generated source forwards every other parameter unchanged to the generated Review,
// so its deep links keep working.

export const MIN_WIDTH = 240
export const MAX_WIDTH = 3840
/** The narrowest preset: the foundations' mobile breakpoint starts at 0px, which no device has. */
export const PHONE_WIDTH = 360

export type Width = number | 'fill'
export type WidthPreset = { id: string; label: string; width: Width }
export type View = 'component' | 'foundations' | 'hud'

export type Route =
  | {
      source: 'runtime'
      view: View
      name: string
      width: Width
      /** The requested page when it does not exist; the route then shows `name`. */
      missing?: string
      /** The export requested by its own name, shown on its owning page `name`. */
      alias?: string
    }
  | { source: 'generated'; query: string }

/** The HUD's pages (the `hud` parameter), the one a missing name falls back to first, and its aliases. */
export type HudPages = { pages: readonly string[]; aliases?: Readonly<Record<string, string>> }

/** One preset per foundations breakpoint (at its minimum width, the phone width for 0), then fill. */
export function widthPresets(breakpoints: Readonly<Record<string, number>>): WidthPreset[] {
  return [
    ...Object.entries(breakpoints)
      .sort(([, a], [, b]) => a - b)
      .map(([id, min]) => {
        const width = Math.max(min, PHONE_WIDTH)
        return { id, label: `${id} ${width}px`, width }
      }),
    { id: 'fill', label: 'Fill', width: 'fill' as const }
  ]
}

/** A `width` parameter: a whole number of pixels within the bounds, or `fill` for anything else. */
export function parseWidth(value: string | null): Width {
  if (value === null || !/^\d+$/.test(value)) return 'fill'
  const width = Number(value)
  return width >= MIN_WIDTH && width <= MAX_WIDTH ? width : 'fill'
}

/**
 * The page each export opens when it is not a page of its own: the first page that lists it, so an export shown on
 * another page (EventCardHost on EventCard) opens by its own name.
 */
export function pageAliases(
  pages: readonly { name: string; components: readonly string[] }[]
): Record<string, string> {
  const names = new Set(pages.map((page) => page.name))
  const aliases: Record<string, string> = {}
  for (const page of pages)
    for (const name of page.components)
      if (!names.has(name) && !Object.hasOwn(aliases, name)) aliases[name] = page.name
  return aliases
}

/**
 * `components` are the page names; `aliases` maps an export shown on another page (EventCardHost) to that page, so
 * it opens by its own name. `hud` names the HUD's pages the same way.
 */
export function parseRoute(
  search: string,
  components: readonly string[],
  foundations: readonly string[],
  aliases: Readonly<Record<string, string>> = {},
  hud: HudPages = { pages: [] }
): Route {
  const params = new URLSearchParams(search)
  if (params.get('source') === 'generated') {
    params.delete('source')
    return { source: 'generated', query: params.toString() }
  }
  const width = parseWidth(params.get('width'))
  const foundation = params.get('foundations')
  if (foundation !== null) {
    const name = foundations.includes(foundation) ? foundation : foundations[0]!
    return {
      source: 'runtime',
      view: 'foundations',
      name,
      width,
      ...(name === foundation ? {} : { missing: foundation })
    }
  }
  const section = params.get('hud')
  if (section !== null && hud.pages.length) {
    if (hud.pages.includes(section)) return { source: 'runtime', view: 'hud', name: section, width }
    if (hud.aliases && Object.hasOwn(hud.aliases, section))
      return { source: 'runtime', view: 'hud', name: hud.aliases[section]!, width, alias: section }
    return { source: 'runtime', view: 'hud', name: hud.pages[0]!, width, missing: section }
  }
  const component = params.get('component')
  if (component !== null && !components.includes(component) && Object.hasOwn(aliases, component))
    return {
      source: 'runtime',
      view: 'component',
      name: aliases[component]!,
      width,
      alias: component
    }
  const name = component !== null && components.includes(component) ? component : components[0]!
  return {
    source: 'runtime',
    view: 'component',
    name,
    width,
    ...(component === null || name === component ? {} : { missing: component })
  }
}

/** A gallery page link, relative to the gallery page. */
export function galleryHref(params: Record<string, string | number | undefined>): string {
  const query = new URLSearchParams()
  for (const [key, value] of Object.entries(params))
    if (value !== undefined && value !== 'fill') query.set(key, String(value))
  const text = query.toString()
  return text ? `?${text}` : './'
}

/**
 * The canvas page for a runtime route, relative to the gallery page: the HUD's sections render in their own document,
 * which loads the HUD's tokens and host styles.
 */
export function canvasHref(view: View, name: string): string {
  return `${view === 'hud' ? 'hud' : 'frame'}.html?${new URLSearchParams({ [view]: name }).toString()}`
}

/** The generated Review page for a forwarded query. `review` is its URL, ending in `/`. */
export function generatedHref(review: string, query: string): string {
  return query ? `${review}?${query}` : review
}
