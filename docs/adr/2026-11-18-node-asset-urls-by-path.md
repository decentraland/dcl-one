# ADR: Serve mirrored assets on paths of the node's public origin

- Status: Accepted
- Date: 2026-11-18

## Context

An asset URL on a per-bucket subdomain requires a DNS record and a TLS certificate for that hostname on each deployment. The node already serves event posters, parcel and world thumbnails, and map images at paths on its public origin.

## Decision

The explore API projects recognised mirrored asset URLs onto the node's public origin at read time. Event posters use `/events-assets-099ac00/poster/` and `/events-assets-099ac00/poster-vertical/`. Parcel thumbnails use `/content/contents/`, world thumbnails use `/worlds-content-server/contents/`, and map images use `/api/v1/map.png` or `/api/v2/map.png` with the original query.

Production event jump links on `decentraland.org` use the node's public origin with their original path and query. Rewriting applies when the node has a public domain outside `decentraland.org`. Decentraland domains, IP addresses, single-label hosts and absent public URLs retain the upstream addresses.

For a recognised world thumbnail, Explore reads the catalogue key with a join on the world name after selecting the place page. Both canonical CIDs and legacy lowercase SHA-256 keys follow the worlds content route's validation. Keys for the same raw CIDv1 stay as projected. Canonical world-scene rows already read the catalogue key with their deployment and project it on the worlds content path when a rewrite domain is set.

## Consequences

The catalogue keeps upstream poster URLs and router-local upload keys. Uploaded posters use the same prefixed paths on reads and upload responses, with root-relative URLs when no public rewrite domain is configured. Authoring recognizes the mounted path and retains the existing stored value when a caller echoes the read projection. The root `/poster` and `/poster-vertical` routes continue to serve operators that configure a dedicated `events-assets-099ac00.<domain>` vhost.

The node's existing asset handlers determine whether each asset is available. The path mapping does not copy world thumbnails into storage.
