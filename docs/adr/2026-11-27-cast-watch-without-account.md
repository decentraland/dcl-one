# ADR: Cast viewers can watch without an account

Status: Accepted

## Context

Public Cast links should let a viewer watch without creating an account or signing
in. The watcher-token endpoint requires a wallet signature and checks that wallet's
platform bans, scene bans and world access. A world with several scenes also needs
a parcel to identify which scene the viewer wants to watch.

## Decision

The Sites watcher uses the signed-in identity when available. Otherwise, it creates
a temporary browser identity for watcher requests, with a one-hour expiration,
renewed within five minutes of expiry. The identity lives only in the tab's memory
and never becomes the account session or enters browser storage. The server's
signature and access checks remain in force.

A world watcher link with no parcel reads the first 100 scenes. A single scene is
selected automatically; several scenes require the viewer to choose before
joining. An explicit parcel takes precedence. A failed or empty scene lookup leaves
resolution to the watcher-token endpoint.

## Consequences

Signed-out viewers need no account. Address bans still apply to signed-in viewers,
but a banned account can watch a public Cast by signing out and receiving a new
address. This moderation limitation is accepted. A private world's access policy
still applies to the temporary address.

Worlds with more than 100 scenes need an explicit parcel link for a scene outside
the selection. Scene lookup failures can still leave a multi-scene link unresolved.
