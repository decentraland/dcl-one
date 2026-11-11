# UI component adoption

This is dcl-one’s adoption tracker. Check the local runtime contract before importing a component; a generated review preview or a copied file does not establish an application API.

## Available contracts

The authoritative public surface is [library/ui/index.ts](../library/ui/index.ts), with the NavBar bridge's accepted limitations in the [promotion registry](../library/ui/components/promotion.json). This table records adoption work rather than duplicating the generated component manifest.

| Component | Runtime contract                                         | Limitations                                                                                                                                  | Verified app adoption |
| --------- | -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- |
| NavBar    | [NavBar.tsx](../library/ui/components/NavBar/NavBar.tsx) | Signed-in placeholders, empty signed-in mobile variants, pending semantic/data contracts and logo sizing. See the registry for exact limits. | Not recorded          |

## Review-only components

The [local review manifest](../library/ui/figma/review/src/generated/components/manifest.ts) owns preview names, node identities, variants and `needsManual` flags. It is not an application import surface. Do not duplicate its records here or infer visual parity from a passing build.

To make another component available, write it by hand from the component template in [UI standards](ui-standards.md), then confirm its public export and mapping. Generated review previews are not a source for public components. A hand-written component records its accepted limits and unresolved blockers in its row of the table above; the resolver repeats limits (`resolved.limits`) only for registry entries.

## Pending work

Requested component contracts and their implementation PRs are tracked in [the shared UI backlog](https://github.com/decentraland/dcl-one/issues/644). Keep planning entries there instead of copying design identities, variant inventories or unverified blocker records into this document. The generated review manifest and local public contracts remain authoritative.

## Application setup

Load shared foundations once per application, from the global entry where one exists. Consumers that honor package exports can use `@dcl/one-ui/foundations.css`. Sites currently loads `@dcl/one-ui/foundations/foundations.css` from its home and download route modules and uses a directory alias; preserve that working setup or explicitly map exported CSS subpaths in both TypeScript and the bundler. Sites and the Explorer HUD already consume shared foundations. Sites also uses app-scoped modules under `library/ui/apps/sites/`; those are separate from this shared-component backlog.

Import shared public contracts from `@dcl/one-ui`. Do not import components from `figma/review/src/generated/` or an unpublished local copy.

## Recording adoption

The author of each verified port or consumer migration updates this tracker in the same PR. For an available contract, record the exact local implementation and export, plus its accepted limits. For each application adoption, record the consumer path, route, verification date/result and full PR URL. “Not recorded” means no verified adoption evidence is recorded here; it is not proof that no consumers exist. Reviewers check these claims against local code and the port verification record.
