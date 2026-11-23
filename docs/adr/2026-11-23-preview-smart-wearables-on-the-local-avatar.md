# ADR: A creator's preview smart wearable is worn by the local avatar only, from the preview listing

- Status: Accepted
- Date: 2026-11-23

## Context

`dcl-scene-cli start` on a smart-wearable project serves the wearable the way
`@dcl/sdk-commands` does: as a `wearable` entity, listed by `/preview-wearables`, and added to
the player's profile on the preview's `/lambdas/profiles`. The Explorer runs the wearable's
scene from the listing, so a creator can interact with it and reload it on save.

The Explorer does not take the avatar from the preview. It reads profiles from the centralized
catalyst and announces only deployed versions ([profiles ADR](2026-11-04-explorer-profiles-from-the-catalyst.md)),
and it resolves wearable definitions from that catalyst. A preview wearable exists only on the
creator's machine, under an id no content server knows, so neither path can put it on the avatar.

## Decision

- **The listing defines the wearable.** Each `/preview-wearables` entry becomes a full wearable
  definition (category, hides, one representation per body shape with its model under that
  body's folder), built by the same code that builds catalyst wearables.
- **It has its own id collection.** The wearable is registered under
  `urn:decentraland:off-chain:preview:{content digest}`, its files in a collection whose base URL
  is the preview's. No catalyst or content server is asked for that id.
- **Only the local avatar wears it.** The preview wearables go first in the primary user's
  rendered avatar shape, so each takes its category's slot; when the preview ends the avatar
  shape is the profile's again. The profile is not changed: comms announcements, what scenes see
  of the player and server-side avatar snapshots still come from the deployed profile.
- **A save is a new wearable.** The id follows the content, so a changed file gives a new id and
  the avatar rebuilds with the new model.

## Consequences

- A creator sees the preview wearable on their own avatar in the web and desktop Explorer, and
  other players never do.
- A scene that sets the player's avatar shape still overrides the preview.
- A listing entry with no body shapes gets no model; its scene still runs.
