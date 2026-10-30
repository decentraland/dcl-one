// from: shop@94584a5c:app/src/lib/searchFacets.ts
// A query that is exactly a category or rarity name (an explicit alias, never a substring) also offers
// that category or rarity, as the sidebar would. Local, no request.

import type { IconName } from "../components/Icon";
import { SHOP_BASE } from "../../routes";
import { CATEGORIES } from "./categories";
import { RARITIES } from "./rarity";
export type Facet =
  | {
      kind: "category"
      // The sidebar's key, which is also what the URL carries: "Hat", "Upper Body", "Dance", or a top
      // category's own key ("wearable", "emote") when the facet is the whole category.
      key: string
      top: string
      labelKey: string
      icon?: IconName
      // The label keys of what the category sits under, outermost first, for the row's subline.
      parents: string[]
    }
  | { kind: "rarity"; key: string }

export const MAX_FACETS = 2

export function facetId(facet: Facet): string {
  return `${facet.kind}:${facet.key}`
}

/**
 * The one normalization on both sides of the match: NFC, lower case, no diacritics, trimmed, single
 * spaces. Small and explicit on purpose: it is not the highlighter's folding and it cuts no punctuation,
 * so "t-shirt" and "tshirt" are two aliases, not one rule.
 */
export function normalizeAlias(text: string): string {
  return text
    .normalize("NFC")
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .normalize("NFC")
    .trim()
    .replace(/\s+/g, " ")
}

// Written 2026-09-22 from the category and rarity labels in both languages plus everyday synonyms and
// plurals, listed one by one. English category words are what the 90 days of searches actually held
// (top, dance, dress, pants, eyes, boots, hair, shirt, shoes, sneakers, hoodie, hat, mask, helmet,
// torso, skirt); Spanish and the rarities are anticipated coverage, with no searches measured yet.
// "top" is Upper Body alone; Top Head answers only to its full name.
export const CATEGORY_ALIASES: Readonly<Record<string, readonly string[]>> = {
  wearable: ["wearable", "wearables"],
  emote: ["emote", "emotes"],
  Head: ["head", "cabeza"],
  "Facial Hair": ["facial hair", "beard", "beards", "mustache", "moustache", "vello facial", "barba", "bigote"],
  Hair: ["hair", "hairstyle", "pelo", "cabello", "peinado"],
  Eyes: ["eyes", "ojos"],
  Eyebrows: ["eyebrows", "cejas"],
  Mouth: ["mouth", "lips", "boca", "labios"],
  "Upper Body": [
    "upper body",
    "top",
    "tops",
    "torso",
    "shirt",
    "shirts",
    "t-shirt",
    "t-shirts",
    "tshirt",
    "tshirts",
    "tee",
    "jacket",
    "jackets",
    "hoodie",
    "hoodies",
    "sweater",
    "sweaters",
    "coat",
    "coats",
    "dress",
    "dresses",
    "camisa",
    "camisas",
    "remera",
    "remeras",
    "camiseta",
    "camisetas",
    "campera",
    "camperas",
    "chaqueta",
    "chaquetas",
    "sudadera",
    "buzo",
    "vestido",
    "vestidos",
    "abrigo"
  ],
  Handwear: ["handwear", "hands", "gloves", "glove", "manos", "guantes"],
  "Lower Body": [
    "lower body",
    "pants",
    "trousers",
    "jeans",
    "shorts",
    "skirt",
    "skirts",
    "leggings",
    "pantalon",
    "pantalones",
    "falda",
    "faldas",
    "piernas",
    "calzas"
  ],
  Feet: [
    "feet",
    "shoes",
    "shoe",
    "sneakers",
    "sneaker",
    "boots",
    "boot",
    "sandals",
    "socks",
    "pies",
    "zapatos",
    "zapato",
    "zapatillas",
    "zapatilla",
    "botas",
    "bota",
    "sandalias",
    "calzado",
    "medias"
  ],
  Accessories: ["accessories", "accessory", "accesorios", "accesorio"],
  Earring: ["earring", "earrings", "aros", "aretes", "pendientes"],
  Eyewear: ["eyewear", "glasses", "sunglasses", "lentes", "gafas", "anteojos"],
  Hat: ["hat", "hats", "cap", "caps", "sombrero", "sombreros", "gorra", "gorras", "gorro", "gorros"],
  Helmet: ["helmet", "helmets", "casco", "cascos"],
  Mask: ["mask", "masks", "mascara", "mascaras"],
  Tiara: ["tiara", "tiaras", "crown", "crowns", "corona", "coronas"],
  "Top Head": ["top head", "parte superior de la cabeza"],
  Skins: ["skin", "skins"],
  Dance: ["dance", "dances", "dancing", "baile", "bailes", "bailar"],
  Stunt: ["stunt", "stunts", "truco", "trucos", "acrobacia"],
  Greetings: ["greetings", "greeting", "saludos", "saludo"],
  Fun: ["fun", "divertido", "diversion"],
  Poses: ["poses", "pose"],
  Reactions: ["reactions", "reaction", "reacciones", "reaccion"],
  Horror: ["horror", "terror"],
  Miscellaneous: ["miscellaneous", "misc", "miscelaneos", "miscelanea"]
}

export const RARITY_ALIASES: Readonly<Record<string, readonly string[]>> = {
  common: ["common", "comun"],
  uncommon: ["uncommon", "poco comun"],
  epic: ["epic", "epico", "epica"],
  rare: ["rare", "raro", "rara"],
  legendary: ["legendary", "legendario", "legendaria"],
  exotic: ["exotic", "exotico", "exotica"],
  mythic: ["mythic", "mythical", "mitico", "mitica"],
  unique: ["unique", "unico", "unica"]
}

// Every facet the sidebar and the rarity filter can express, keyed the way the URL carries them.
function categoryFacets(): Map<string, Facet> {
  const out = new Map<string, Facet>()
  for (const top of CATEGORIES) {
    if (top.key === "all" || top.key === "names") continue
    out.set(top.key, { kind: "category", key: top.key, top: top.key, labelKey: top.labelKey, parents: [] })
    for (const sub of top.subs ?? []) {
      out.set(sub.key, {
        kind: "category",
        key: sub.key,
        top: top.key,
        labelKey: sub.labelKey,
        icon: sub.icon,
        parents: [top.labelKey]
      })
      for (const leaf of sub.subs ?? [])
        out.set(leaf.key, {
          kind: "category",
          key: leaf.key,
          top: top.key,
          labelKey: leaf.labelKey,
          icon: leaf.icon,
          parents: [top.labelKey, sub.labelKey]
        })
    }
  }
  return out
}

export const FACETS: readonly Facet[] = [
  ...categoryFacets().values(),
  ...RARITIES.map((key): Facet => ({ kind: "rarity", key }))
]

// alias → facet. An alias listed under a key the sidebar does not know is dropped here (and caught by
// the tests); the first facet to claim an alias keeps it.
const BY_ALIAS = new Map<string, Facet>()
for (const facet of FACETS) {
  const aliases = facet.kind === "category" ? CATEGORY_ALIASES[facet.key] : RARITY_ALIASES[facet.key]
  for (const alias of aliases ?? []) {
    const normalized = normalizeAlias(alias)
    if (!BY_ALIAS.has(normalized)) BY_ALIAS.set(normalized, facet)
  }
}

/** The facets a query names: the whole query, normalized, against the aliases. At most MAX_FACETS. */
export function facetsFor(query: string): Facet[] {
  const normalized = normalizeAlias(query)
  if (normalized.length < 2) return []
  const facet = BY_ALIAS.get(normalized)
  if (!facet) return []
  const unique = new Map<string, Facet>([[facetId(facet), facet]])
  return [...unique.values()].slice(0, MAX_FACETS)
}

/**
 * The destination, built from nothing but the facet: the filters the sidebar would set and no others,
 * so nothing from the page the reader is on contradicts it. Without a query the grid keeps its own
 * default status (on sale).
 */
export function facetUrl(facet: Facet): string {
  const params = new URLSearchParams()
  if (facet.kind === "rarity") {
    params.set("rarities", facet.key)
  } else {
    params.set("category", facet.top)
    if (facet.key !== facet.top) params.set("subCategory", facet.key)
  }
  return `${SHOP_BASE}/items?${params.toString()}`
}
