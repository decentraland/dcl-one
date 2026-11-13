# UI component adoption

This is dcl-one’s adoption tracker. Check the local runtime contract before importing a component; a generated review preview or a copied file does not establish an application API.

## Available contracts

The authoritative public surface is [library/ui/index.ts](../library/ui/index.ts). Public components are hand-written, and this table records their open limitations and adoption.

| Component | Runtime contract                                         | Limitations                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Verified app adoption                                                                                                                                                                                                                                                                                                                                      |
| --------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NavBar    | [NavBar.tsx](../library/ui/components/NavBar/NavBar.tsx) | Hand-written. No light theme (theme selection is undecided). The app supplies the notifications panel, account data and balances; NavBar renders only what it is given. Below 768px balances show only in the menu drawer. The drawer renders inside the bar, so a consumer stacking context (a sticky header with a `z-index`) also contains it, and the page behind relies on `aria-modal` instead of `inert`. Not verified with a screen reader or on a touch device. | Sites adoption in [NavBar integration](https://github.com/decentraland/dcl-one/pull/687): `apps/sites/src/features/components/chrome/SitesTopBar.tsx`, root and native Shop entry points. Local signed-out responsive layout and synthetic account/credits states verified; live auth, IAP, screen-reader and physical-touch acceptance remain unverified. |

The mobile menu keeps the single branded header usable above a full-width lower panel. Opening the menu moves numeric balances into that panel at all mobile widths; account and balance controls are not duplicated. The public `className` applies to the outer navigation frame so host positioning and stacking remain effective.

NavBar integration follow-ups: consolidate the legacy notification inbox cache with the Sites private-key lifecycle, and retire the legacy `overlayNav` body flag when its consumers migrate. The current native header surface remains explicit so existing body layouts keep their behavior. The Sites geometry test currently requires Chromium in the default test command; separating it requires preserving explicit CI browser coverage. Local CI path selection also needs shared-component and Sites-library coverage. These tooling follow-ups do not certify live authentication or service acceptance.

## Review-only components

The [local review manifest](../library/ui/figma/review/src/generated/components/manifest.ts) owns preview names, node identities, variants and `needsManual` flags. It is not an application import surface. Do not duplicate its records here or infer visual parity from a passing build.

To make another component available, write it by hand from the component template in [UI standards](ui-standards.md), then confirm its public export and mapping. Generated review previews are not a source for public components. A hand-written component records its accepted limits and unresolved blockers in its row of the table above; the resolver repeats limits (`resolved.limits`) only for registry entries.

## Pending work

Requested component contracts and their implementation PRs are tracked in [the shared UI backlog](https://github.com/decentraland/dcl-one/issues/644). Keep planning entries there instead of copying design identities, variant inventories or unverified blocker records into this document. The generated review manifest and local public contracts remain authoritative.

## Application setup

Load shared foundations once per application, from the global entry where one exists. Consumers that honor package exports can use `@dcl/one-ui/foundations.css`. Sites loads `@dcl/one-ui/foundations.css` once from `apps/sites/src/routes/root.tsx`; its bundler maps that exported CSS subpath and CSS side-effect imports use the ambient CSS module declarations. Sites and the Explorer HUD already consume shared foundations. Sites also uses app-scoped modules under `library/ui/apps/sites/`; those are separate from this shared-component backlog.

Import shared public contracts from `@dcl/one-ui`. Do not import components from `figma/review/src/generated/` or an unpublished local copy.

## Recording adoption

The author of each new public component or consumer migration updates this tracker in the same PR. For an available contract, record the exact local implementation and export, plus its accepted limits. For each application adoption, record the consumer path, route, verification date/result and full PR URL. “Not recorded” means no verified adoption evidence is recorded here; it is not proof that no consumers exist. Reviewers check these claims against local code and its tests, and against the applicable port verification record.
