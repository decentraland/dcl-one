# ADR: Shared UI imports through the package entry

- Status: Accepted
- Date: 2026-11-23
- Amends: [Shared UI import name](2026-11-09-shared-ui-import-name.md), [Shared app chrome contracts](2026-11-20-app-chrome-contracts.md)

## Context

`@dcl/one-ui/<path>` resolves to `library/ui/<path>` in every consumer, so apps imported shared components and foundations by file path, and the app chrome contracts were imported module by module. A deep path couples a consumer to the library's file layout and skips the public surface that `library/ui/index.ts` declares.

## Decision

Apps, other libraries and tooling import shared UI only from `@dcl/one-ui`, never from a subpath. `library/ui/index.ts` exports the app chrome contracts (`chrome-auth`, `chrome-nav`, `thumbnail-public-base`, `ChromeBody`, `ChromeLink`, `SiteLayout` and `useSiteLayout`); providers and consumers still share one context, because the entry re-exports the same modules the library imports internally. The app-scoped UI under `library/ui/apps/<app>/` follows the same rule once the entry exports it.

Stylesheets keep their paths (`foundations.css`, `text-styles.module.css` and the token sheets), because a script entry cannot carry a CSS Module or a global sheet. The Explorer HUD keeps its `@dcl/one-ui/foundations` entry.

## Consequences

- `tooling/ui_standards.py --shared-imports` fails on any other `@dcl/one-ui/<path>` module in `apps/`, `library/` (outside `library/ui`) and `tooling/`; `make check` and the structure workflow run it. It accepts `@dcl/one-ui/apps/` paths until the entry exports those modules.
- A module a consumer needs from `library/ui` is exported from `library/ui/index.ts` first.
