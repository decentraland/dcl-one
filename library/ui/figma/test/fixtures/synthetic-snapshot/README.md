# Synthetic snapshot

Hand-written synthetic data for the unit tests. It is **not a Figma capture** and was not derived from one: every node id (`10:1`, `20:2`, ...), page, component, layer, token, text style, icon and image is invented, and the rasters are tiny generated PNGs with noise pixels. It holds no Figma file key or URL. The real capture is a gitignored cache that `pnpm figma fetch-source` materializes for maintainers; tests never read it.

The files follow the layout `SnapshotSource` reads, so the code under test parses them exactly as it parses a capture: `pages.json`, `sync-state.json`, `live-source.json`, `generation-inventory.json`, `variables.json`, `text-styles.json`, `paint-styles.json`, `icons.json` with `icons/*.svg`, and one `components/<encoded page name>.json` per page. `../figma.config.json` configures these pages, and `../demo-nav-text-roles.json` holds the fixture ids the text-role tests write onto the DemoNav views.

## Foundations

- `variables.json`: four collections, Primitives (10 colours, one with alpha), Semantic (5 aliases), Spacing (5 `space/*`, 3 `radius/*`) and Layout (4 modes with a breakpoint, a margin, a max width and two responsive font sizes that change at the tablet breakpoint only).
- `text-styles.json`: 7 styles, including upper-case label and button styles. `paint-styles.json`: one linear and one radial gradient.
- `icons.json`: 22 single icons and one `active=false/true` pair in two categories, plus one icon that embeds a raster and one with an unsupported variant, both of which the generator must skip.

## Component pages

| Page (id) | What it exercises |
| --- | --- |
| Demo Controls (`10:1`) | `Synthetic Button`: a `Property 1` enum with a misspelt option (`Terciary`), `Hover` and `Blocked` yes/no axes, token fills and a measured label line height. `Switch` (On/Off boolean, reserved prop name, loading state), `HideItem` (On/Off boolean), `FriendCTA` (option aliases, hover and pressed typos, an unresolved instance and a per-variant icon alias), `Split Button` (a skipped option and colliding aliases), `Notify` (vector artwork), `SquaredButton` (a hover that adds a layer), `InterestedCTA` (a library icon), `Txt / Sort` (a configured component name and a legacy icon alias) and `Caret Input` (a text caret). Not in the inventory. |
| Demo Toggles (`10:2`) | `SwitchTitle`: an instance of `Switch` from another page, and space-between distribution. |
| Demo Cursors (`10:3`) | Checksummed SVG artwork at absolute offsets, PNG artwork carrying a `gAMA` chunk, artwork too heavy to embed, a three-variant set, and a `Badge` whose name collides with the one on Demo Feedback. |
| Demo Feedback (`10:4`) | A wrapping fixed-width text, a legacy text style alias, a truncated capture, a Figma slot property, an image fill with its PNG in the `images` table, negative item spacing and the other `Badge`. |
| Demo Nav (`10:7`) | `DemoNav`: 18 of the 36 combinations of four axes (`viewport`, `signed-in`, `balances`, `theme`), each non-default variant a whole-tree replace, with the boxes its configured semantics name; and a raster carrying an `iCCP` chunk. |
| Demo: Legacy Bars | A legacy capture configured by name only, whose embedded page id is Demo Nav's. Its name holds a `:` that encodes to `%3A` in the file name. |
| Demo Dialogs (`10:8`) | `Dialog`, the only component the inventory publishes from this page (Review only, through a configured selection exception), and `Sheet`, which it leaves out. |
| Demo Field (`10:5`), Demo Search (`10:6`) | Ready pages for the single-page live refresh: the first is refreshed, the second must keep its capture. |

`sync-state.json` marks Demo Feedback, Demo Nav, Demo Field and Demo Search ready; the inventory publishes those four pages in both scopes and only `Dialog` from Demo Dialogs, in Review.
