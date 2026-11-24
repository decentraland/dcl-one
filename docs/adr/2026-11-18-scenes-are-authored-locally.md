# ADR: Scenes are authored locally; the node keeps no scene projects

- Status: Accepted
- Date: 2026-11-18

## Context

Creators author a scene as local files, in the desktop Creator Hub (`apps/creator-hub`) or with
`dcl-scene-cli`, and publish it to a World or to LAND from there. A second copy of the scene
kept on the node, together with a browser editor to work on it, duplicates that workflow and
leaves the node holding state no other deliverable reads.

The builder area has another job: collections, items, their files and their curation, the
wearable and emote side of `/create`.

## Decision

- **The node keeps no scene projects.** The builder serves no project, manifest, project file,
  asset pack, asset catalogue or asset content route, and the schema, settings and operator
  commands for them do not exist. A request to an upstream scene route is not answered. The
  builder area serves collections, items, their files, curation, third parties and the small
  reads around them. Deployed scenes are unaffected: the `content` area stores deployments and
  the `worlds` area hosts world scenes.
- **Web `/create` is the wearable and emote builder plus the management of what is already
  deployed.** It has no scene editor, no browser scene projects, no scene templates and no web
  publishing of a scene. It keeps the collection and item pages and the pages for worlds,
  names, storage, LAND, metrics, scene analytics, curation, multiplayer preview and the list of
  published scenes. Its "Start building" action opens the Creator Hub download page.
- **`dcl-scene-cli` works on local files.** It has no `sync` command and no client for node
  scene routes.
- **The desktop Creator Hub workspace is where scenes are authored and published.**
  `apps/creator-hub` is an Electron app with the inspector and the asset packs.

## Consequences

- A creator cannot publish a scene to a World or to LAND from web `/create`. The desktop Creator
  Hub's publish dialog runs `dcl-scene-cli`'s `deploy` in programmatic mode
  (`apps/creator-hub/packages/creator-hub/main/src/modules/cli.ts`), and `dcl-scene-cli deploy`
  signs and uploads to a world or to a catalyst from the command line
  ([README](../../apps/dcl-scene-cli/README.md)).
- A scene a creator kept only as a server project, or only as a browser draft, is not reachable
  from web `/create`. A scene already deployed stays listed on the published list, where its
  files can be downloaded.
- A client of the upstream builder service that calls its scene or asset pack routes gets no
  answer from the node.
- A database that holds the retired scene tables keeps them until an operator runs the release
  script `schemas/releases/builder-scenes-retired.sql`; the drop is not reversible without a
  backup ([`DATABASE.md`](../../DATABASE.md), "Retired builder scenes").
- Code that serves a server-side scene project, a web scene editor or web scene publishing serves none
  of the deliverables in the `AGENTS.md` scope list and is removed, not rebuilt.
