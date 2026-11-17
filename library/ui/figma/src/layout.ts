// figma-design-system keeps the tool as three packages at its repository root: dcl-mcp-figma (the CLI
// and capture), dcl-ui-review (the Review) and component-library (the published library). Here they
// live in library/ui, the root every repo-relative path below is resolved against (app.cwd/..):
//   dcl-mcp-figma                              → figma
//   dcl-ui-review                              → figma/review
//   component-library/src/generated/components → components (promoted components, one directory each)
//   component-library/src                      → components (hand-written components)
//   component-library/src/index.ts             → index.ts (the public entry)
//   component-library/promotion.json           → components/promotion.json
//   component-library/package.json             → package.json
// The library entry re-exports promoted components relative to itself: upstream ./generated/components/…
// (from component-library/src/index.ts) is ./components/… here (from index.ts).
// The ported sources use these mapped paths directly; layout.test.ts fails on any other repo-root path
// of the upstream layout.
const UPSTREAM_LAYOUT = [
  ['dcl-mcp-figma', 'figma'],
  ['dcl-ui-review', 'figma/review'],
  ['component-library/src/generated/components', 'components'],
  ['component-library/src', 'components'],
  ['component-library/src/index.ts', 'index.ts'],
  ['component-library/promotion.json', 'components/promotion.json'],
  ['component-library/package.json', 'package.json'],
  ['./generated/components/ (library entry)', './components/']
] as const

export { UPSTREAM_LAYOUT }
