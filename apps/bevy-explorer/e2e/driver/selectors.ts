// How a spec names a HUD control: the accessible name, or a JSON selector as `/hud_*` take it
// (react-web/src/lib/hudAutomation.ts): `name` matches case-insensitively after whitespace
// normalisation, exactly unless `exact: false`; `role` filters; `index` picks among several matches.

export interface SelectorObject {
  name?: string
  role?: string
  index?: number
  /** `false` matches `name` as a substring. */
  exact?: boolean
}

export type Selector = string | SelectorObject

export function toSelectorObject(selector: Selector): SelectorObject {
  return typeof selector === 'string' ? { name: selector } : { ...selector }
}

/** The selector as the console line carries it: a bare name, or JSON when it has more than a name.
 *  A name that looks like JSON goes as JSON too, so the page never parses it as a selector. */
export function selectorArg(selector: Selector): string {
  const object = toSelectorObject(selector)
  const keys = Object.keys(object).filter((key) => object[key as keyof SelectorObject] !== undefined)
  const onlyName = keys.length === 1 && keys[0] === 'name' && object.name !== undefined
  if (onlyName && !looksLikeJson(object.name!)) return object.name!
  return JSON.stringify(object)
}

function looksLikeJson(name: string): boolean {
  const trimmed = name.trim()
  return trimmed.startsWith('{') || trimmed.startsWith('"') || trimmed.startsWith('[') || /^-?\d/.test(trimmed)
}

export function describeSelector(selector: Selector): string {
  return typeof selector === 'string' ? `"${selector}"` : JSON.stringify(selector)
}
