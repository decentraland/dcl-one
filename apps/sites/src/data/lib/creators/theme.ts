// The design values TypeScript reads at runtime. Everything a stylesheet draws with is a custom property in
// tokens.css; only what code needs as a value lives here.

// Per-rarity design colors (Figma "Rarities/*", shop's palette), for the media wash computed per item.
// Distinct from @dcl/schemas' Rarity.getColor: the designer re-tuned every one for the dark field.
export const rarities = {
  common: '#73d3d3',
  uncommon: '#ff8362',
  rare: '#34ce76',
  epic: '#289cff',
  legendary: '#a24bf3',
  exotic: '#9cd71e',
  mythic: '#ff4bed',
  unique: '#fea217'
} as const

// Canonical breakpoints, for `useMediaQuery` and the literal widths of every stylesheet's media queries
// (src/styles/breakpoints.spec.ts). `mobile` (768) is the primary one.
export const breakpoints = {
  mobile: 768,
  sm: 720,
  md: 820,
  lg: 900,
  xl: 1200,
  // decentraland-ui2's MUI breakpoints (sm/md/lg), which the overview page inherits from sites.
  tablet: 991,
  laptop: 1024,
  desktop: 1280
} as const

export type Breakpoint = keyof typeof breakpoints

export const media = {
  maxWidth: (bp: Breakpoint) => `@media (max-width: ${breakpoints[bp]}px)`,
  // Exact complement of maxWidth. `min-width: bp + 1` leaves a gap for fractional viewport widths
  // (browser zoom, DPR scaling) where neither query matches and unstyled layout leaks through.
  minWidth: (bp: Breakpoint) => `@media not all and (max-width: ${breakpoints[bp]}px)`,
  /** Below this width the app is a viewer: header buttons and row menus that manage collections are hidden. */
  noActions: `@media (max-width: ${breakpoints.lg}px)`
}
