# ADR: The Creator Hub ships as the desktop app only

- Status: Accepted
- Date: 2026-11-19

## Context

Sites served a web Creator Hub under `/create/*`: home, templates, a browser scene editor with
its scene list and drafts, publishing, World admissions, settings and storage, LAND and NAME
lists, metrics and scene analytics, tutorials, a data-sources ledger and a flow map. The desktop
app in `apps/creator-hub` ([ADR](2026-11-09-creator-hub-in-dcl-one.md)) covers the same scene,
World and LAND work, and scenes are no longer created or edited on the web.

## Decision

The Creator Hub is the desktop app. Sites keeps, under `/create`:

- the creators landing (`/create`) and its blog rail (`/create/latest-posts`);
- the wearable and emote collection pages and curation (`/create/wearables/*`, `/create/curate`);
- `/create/preview`, the multiplayer preview other teams test with;
- the `/create/about`, `/create/claim-name` and `/create/collections` redirects.

Every other web Creator Hub page is removed, with the code, data readers, telemetry events,
public assets and APIs only it used (`/api/creator-hub/*`, `/api/screens/v1/create`).

- **Old links.** `/create/<page>` and `/creator-hub/<page>` for a removed page answer a permanent
  redirect: `land` and `names` to the account's assets (`/profile/me/assets`), every other page to
  `/download/creator-hub`; the list of pages is `apps/sites/src/features/lib/creator/retired-hub.ts`.
  Any other unknown `/create/*` path is a 404. `/create?projectUrl=`, the link `dcl-scene-cli start`
  prints, goes to `/download/creator-hub`.
- **Builder links.** `/builder/scenes`, `/builder/worlds`, `/builder/pool` and the scene editor go to
  `/download/creator-hub`; `/builder/land` and `/builder/names` go to the account's assets
  (`/profile/me/assets`), and `/builder/land/:id` to that LAND's Shop manage page.
- **Site links.** Links that sent people to build or publish a scene, or to manage a World, go to
  `/download/creator-hub`: the navigation's Publish LAND, the profile's Manage World, the Shop's
  LAND, rental and NAME pages and, outside the mobile app, the NAME purchase's "Use it for your
  World". A NAME in the profile's assets opens its Shop token page.

## Consequences

- No web page creates, edits or publishes a scene.
- The Explorer's editor host (`library/explorer-core/features/editorHost`), the editor scene
  (`apps/ui3/editor-scene`, served at `/editor-scene/`) and the `dcl-scene-cli start` link to
  `/create?projectUrl=` stay until they are removed or repointed with the Explorer and CLI owners.

## Follow-ups

1. Remove or repoint the Explorer editor host, the editor scene and the `dcl-scene-cli start`
   Creator Hub link.
2. Delete what only the framed web hub and the Tauri shell used in Sites: the Explorer hub host
   (`features/components/chrome/explorer-host.ts`, `@ui/creatorhub/explorer-host`) and the native
   sign-in relay.
