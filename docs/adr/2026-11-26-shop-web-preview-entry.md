# Shop web avatar preview entry

- Status: Accepted
- Date: 2026-11-26
- Amends: [Shared UI imports through the package entry](2026-11-23-shared-ui-entry-imports.md)

## Context

The web Shop embeds an avatar preview iframe in item details, outfits and home promotions. The Explorer hosts the in-world preview and includes the shared Shop root in its input manifest. Exporting the web preview from the root would make the iframe and renderer protocol HUD inputs.

## Decision

The web Shop imports the iframe presentation through `@dcl/one-ui/apps/shop/preview`. Its ItemPreview, AvatarStage and LivePromo adapters and the AvatarStage test may import this entry. The Shop and public UI root entries stay independent of it. Consumers keep the preview's internal modules and stylesheets private.

The app supplies device capabilities, feature flags, the session profile and the router link. Pure renderer selection lives in `library/shop/detail/lib`. The Explorer host supplies its own rendering surface.

## Consequences

The shared-import guard enforces the adapter allowlist and rejects preview-internal imports, including stylesheets, from other packages and the HUD. The package export and directory inventory remain inputs to module resolution. Regenerate the HUD manifest when either changes and compare the shared sources and edges against the base revision.
