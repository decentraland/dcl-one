# UI standards

Normative standards for web UI code in this repository. Each section records a
decision that has been made; areas without a decision say so instead of
guessing. The first standard defined here is styling.

## Contents

- [Relation to the contributor guides](#relation-to-the-contributor-guides)
- [Styling: CSS Modules](#styling-css-modules)
  - [Scope](#scope)
  - [Files and imports](#files-and-imports)
  - [Design tokens](#design-tokens)
  - [Global CSS boundary](#global-css-boundary)
  - [Variants and state](#variants-and-state)
  - [Runtime values](#runtime-values)
  - [Composition](#composition)
  - [Responsive, themes and portals](#responsive-themes-and-portals)
  - [Accessibility](#accessibility)
  - [Migrating existing styles](#migrating-existing-styles)
  - [Verification](#verification)
- [App-specific UI](#app-specific-ui)
- [Shared UI: `library/ui`](#shared-ui-libraryui)
  - [Structure](#structure)
  - [Foundations](#foundations)
  - [Explorer HUD transition](#explorer-hud-transition)
  - [Integration](#integration)

## Relation to the contributor guides

The current contributor guides reflect the standards in production. This
document defines the new standards for One.

## Styling: CSS Modules

**Decision:** component styles are written as CSS Modules.

### Scope

- Applies to every current and future web app under `apps/` and to
  `rendering/bevy-explorer/source/react-web`.
- Required for all new component styles and for existing styles when they are
  migrated. Existing global stylesheets do not have to be migrated all at once.

### Files and imports

- Colocate the module with its component: `Component.tsx` next to
  `Component.module.css`.
- Import the class map and apply classes from it:
  `import styles from './Component.module.css'`, then
  `className={styles.root}`.
- Write ordinary CSS. No preprocessor or CSS-in-JS syntax.
- Do not introduce Emotion, styled-components, MUI `styled`, or another
  CSS-in-JS layer for new styling.

Start a shared component from the reusable template at
`library/ui/templates/component/`. Copy and rename the files into the new
component's directory under `library/ui/components/`:

```text
Component.tsx          React component and its props and types
Component.module.css   component styles
Component.test.tsx     behavior and accessibility tests
Component.figma.ts     Figma-to-component mapping for the tooling
```

Define component-owned props and types in the component file by default. Use a
separate types file for a substantial contract shared across components.
Export the component and its public prop types from `library/ui/index.ts`.
The mapping is used by the Figma tooling and is not imported by browser code.
The template is not a production component. Replace the Figma mapping
placeholder with the actual component's node and properties when its tooling
contract is established. The sample test is a smoke test; configure test globals,
React Testing Library, jest-dom matchers and CSS Module handling, and add the
component-specific interaction and accessibility tests before use.

### Design tokens

Use shared design tokens through CSS custom properties, for example
`color: var(--text)`. Use semantic tokens appropriate to the component's
context and theme rather than hardcoding values or copying them from another
surface. CSS Modules scope class names, not custom properties: ensure the
required token scope is available wherever the component renders, including
portals.

### Global CSS boundary

Shared foundations and components belong in `library/ui`. Global stylesheets
contain only token definitions, font faces, resets, and application-wide base
and accessibility rules. Each app keeps its own global entry point and loads
the shared foundations it needs.

When migrating a shared style or component to `library/ui`, update its
consumers and remove the replaced implementation from the old location.

Component selectors belong in modules. Avoid:

- routine `:global(...)` selectors;
- ancestor overrides such as `.page .button { ... }`;
- selectors that reach into another component's internals, including a
  shared component from `library/ui` styled from an app.

When a legacy global class has to keep working, keep it deliberately, write
down which consumer depends on it, and verify that consumer after the change.

### Variants and state

Control component variants and states through props and component CSS, not
global overrides. Disabled controls must actually be disabled, and existing
states such as focus and hover must be preserved.

### Runtime values

Inline styles are allowed for values that are only known at runtime, such as
progress, position or a computed size. Pass them as a narrowly scoped custom
property and let the module consume it; fixed appearance stays in the module.

```tsx
<div className={styles.track} style={{ '--progress': `${pct}%` } as React.CSSProperties} />
```

Shared components expose runtime geometry through typed props mapped to custom
properties, while their appearance stays in the module. Do not use runtime
values to pass arbitrary colours or palette values from props.

### Composition

- Reuse shared components from `library/ui` before creating a new component.
- Put components shared by multiple apps or the Bevy HUD in `library/ui`.
- Keep feature-specific components and their styles with the feature; see
  [App-specific UI](#app-specific-ui).

### Responsive, themes and portals

- Use the breakpoint definitions in `library/ui` foundations once implemented
  and adopted; retain each surface's breakpoints until then. CSS custom
  properties cannot be used directly in media conditions.
- Select themes through the shared semantic token mechanism. The supported
  themes and the selection mechanism's details remain to be defined.
- Carry the required theme scope into portal content rendered outside its
  original subtree.
- HUD overlays must account for runtime viewport scaling.

### Accessibility

Components need visible focus, keyboard interaction, labels, sufficient
contrast and reduced-motion support. Do not override reduced-motion rules
from a component module.

### Migrating existing styles

A migration covers one bounded component and its consumers, and is scheduled
on purpose. Before changing anything, inventory:

- every reference to the component's global classes, including other
  stylesheets and tests;
- specificity and import order the current look depends on;
- theme scopes and portal contexts the component renders in.

Keep the public props and any legacy class hook until its callers are
migrated. Renaming `component.css` to `component.module.css` is not a
migration. Do not rewrite unrelated components or tokens in the same change.

### Verification

- Run the affected interaction tests and verify each consuming surface.
- Check each applicable theme and representative width in a browser, including
  portals and server-rendered layouts where applicable.
- Compare visual behavior before and after a change; unit tests alone do not
  prove visual parity.
- Report checks that could not run or failed. Documentation-only changes need
  link, structure and content checks, not an application build.

## App-specific UI

App-specific UI is used by one app only: its pages, layouts, app chrome and
feature components. It lives in that app, next to the feature that uses it,
and not in `library/ui`.

- Follow the same CSS Modules, accessibility, migration and verification rules
  as shared UI.
- Compose it from shared components and foundations in `library/ui`. Do not
  restyle a shared component's internals from the app; when the app needs a
  different look, add or extend a variant on the shared component.
- Use shared tokens rather than hardcoded values or values copied from another
  app.
- Each app keeps its own global entry point for its resets and base rules.
- When a second app or the Bevy HUD needs the same component or foundation,
  move it to `library/ui`, update its consumers and remove the app's copy.

## Shared UI: `library/ui`

The structure and integration below define the target for the shared UI. Its
foundations, runtime components and consumer integration still need to be
implemented; the component template is available now. Keep each surface's
existing tokens and components until its integration has been reviewed.

`library/ui` is the home of the shared UI foundation:

- design foundations and tokens shared across surfaces;
- React components used by more than one app, or by an app and the Bevy HUD
  (for example Button);
- the Figma MCP integration.

The Figma MCP integration is Node-only tooling and is never imported by browser
code; the shared runtime is foundations and components, with React as a peer
dependency. "Bevy HUD" here means the React DOM HUD in
`rendering/bevy-explorer/source/react-web`; sharing components with it does
not extend to Rust Bevy widgets or SDK scene UI.

### Structure

```text
library/ui/
  foundations/
    colors/             primitive and semantic colors; gradients
    typography/         font families, sizes and text styles
    layout/             responsive layout values
    spacing/            spacing and corner-radius scales
    icons/              shared icon assets and exports
  components/
    Button/
      Button.tsx
      Button.module.css
      Button.test.tsx
      Button.figma.ts   Figma mapping, read by the tooling only
  index.ts              public exports: components and their prop types
  figma/                Node-only Figma MCP server and CLI, config, snapshots
```

- **Foundations:** expose shared tokens as CSS custom properties and typed
  values. Each app loads shared tokens through its global entry point.
- **Components:** use React and colocated CSS Modules, export public components
  from `index.ts`, and declare React and React DOM as peer dependencies. Each
  consumer bundles the shared source it imports.
- **Figma tooling:** keep the MCP server and CLI in `figma/` and outside browser
  exports. Component mappings connect Figma nodes to shared components.

### Foundations

`library/ui/foundations` owns the design values and assets shared by the UI.
Components consume these foundations rather than defining their own palette,
typography, spacing, or icon set. Keep the following foundation families in
this shared library:

- **Color:** primitive colors and semantic roles for backgrounds, text, actions,
  borders and feedback. Semantic roles reference primitives instead of
  duplicating their values.
- **Spacing and shape:** spacing and corner-radius scales used by components
  and layouts.
- **Layout:** responsive margins, section spacing and typography sizes for
  desktop, large desktop, tablet and mobile contexts.
- **Typography:** font families, weights, sizes, line heights, letter spacing
  and text styles. Supply the font assets and their licenses with the library.
- **Gradients:** named gradient styles and their stops and directions.
- **Icons:** shared icon assets and their variants, with their size, color and
  accessible-use contracts.

Figma supplies the design source for these foundations. The Figma tooling
captures and validates the source before producing the shared library's
foundation outputs. Preserve semantic aliases, modes and units in that
process; a missing or unsupported value must be reported for review instead of
silently substituting a value from another mode. Generated output is reviewed
before it becomes a shared foundation.

Review theme and surface mappings before replacing existing values. Preserve
runtime state such as HUD viewport scaling in the consumer.

### Explorer HUD transition

The Explorer HUD's existing `src/design/` primitives and local tokens remain in
use under its own guide (`rendering/bevy-explorer/source/react-web/AGENTS.md`)
until the shared library integration is reviewed and implemented. A future
migration must preserve HUD behavior.

### Integration

Each consumer keeps its own independent build and imports the shared code as
source:

```tsx
import { Button } from '<shared-ui>'
```

`<shared-ui>` is a placeholder; the shared library's import name remains to
be selected. Each consumer resolves it consistently in TypeScript and its
build, allows access to shared sources and
deduplicates React and React DOM. Assets must work in every supported target,
including native CEF.

Build inputs, dependency resolution, HUD provenance and CI selection must
include the shared sources so that changes invalidate the affected artifacts.
