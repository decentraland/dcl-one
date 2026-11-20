# ADR: The Explorer HUD's Showcase in the UI gallery

- Status: Accepted
- Date: 2026-11-20
- Supersedes in part: [Runtime component gallery in `library/ui/gallery`](2026-11-20-ui-runtime-gallery.md), where it leaves the HUD design primitives out

## Context

The Explorer HUD's design primitives (`library/ui/apps/hud/design`, exported by its `index.ts`) had one review surface: the Showcase (`design/Showcase.tsx`), opened with `?showcase=1` in a HUD host. Each new primitive has to be added to it (`rendering/bevy-explorer/source/react-web/AGENTS.md`), and nothing checked that: BrowseToolbar, the Modal family, Submenu and others had no example of their own. The runtime gallery covered only the components of `@dcl/one-ui`. The HUD compiles under names its two hosts give it (`@hud`, `@explorer-core`, `@hud-host`), with their page styles and the `--ui-scale` they install from the window.

## Decision

The Showcase stays the single source of the HUD examples, and the gallery shows it.

- `design/ShowcaseIndex.ts` names each section, its heading and the components of `design/index.ts` it renders; `Showcase.tsx` renders each section by name, and `?showcase=1` still shows them all. The Showcase imports the primitives from the design entry, as the HUD's features do.
- The gallery lists the sections under HUD, from the index alone, and renders them in their own canvas document (`hud.html`) that loads what a host's page loads: the foundations, the HUD tokens and the hosts' page styles. Its Vite config gives the HUD its hosts' names, with the library's own HUD test host as `@hud-host`. `tsconfig.hud.json` checks that part with the HUD's compiler profile.
- A HUD canvas is a window of the chosen width and 16:9 height (480px at least) that installs `--ui-scale` as the hosts do, so dialogs, toasts and `vh` sizes behave as in a host.
- A test in the `hud` Vitest project fails when a component the design entry exports is named by no section and has no exclusion with a reason, or when a section does not mount, through the design entry, a component it names. The entry's functions and constants are listed with their reasons. Sections use local data only.
- The glyphs (`design/Glyphs.tsx`) are drawn, every one, by a gallery page, as the gallery draws the foundations' icons, and the test checks that page mounts each. A Showcase section would keep the glyphs the HUD does not use in its eagerly loaded design chunk: react-web's build groups every module of `design/` there except the Showcase.

## Consequences

Adding a component to the HUD's design entry requires a Showcase section that names and renders it, or an exclusion with a reason, in the same change. A component only the Showcase uses ships in that design chunk all the same. Changing the Showcase changes HUD sources, so the HUD shared-input manifest is regenerated as for any other HUD edit. The gallery's build compiles HUD code; a HUD import that needs a package `library/ui` does not install fails that build, unless it is reached only through a type import.
