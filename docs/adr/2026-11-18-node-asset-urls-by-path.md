# ADR: Serve mirrored assets on paths of the node's public origin

- Status: Accepted
- Date: 2026-11-18

## Context

An asset URL on a per-bucket subdomain requires a DNS record and a TLS certificate for that hostname on each deployment. The node already serves event posters, parcel and world thumbnails, and map images at paths on its public origin.

## Decision

The explore API projects recognised mirrored asset URLs onto the node's public origin at read time. Event posters use `/events-assets-099ac00/poster/` and `/events-assets-099ac00/poster-vertical/`. Parcel thumbnails use `/content/contents/`, world thumbnails use `/worlds-content-server/contents/`, and map images use `/api/v1/map.png` or `/api/v2/map.png` with the original query.

Production event jump links on `decentraland.org` use the node's public origin with their original path and query. Rewriting applies when the node has a public domain outside `decentraland.org`. Decentraland domains, IP addresses, single-label hosts and absent public URLs retain the upstream addresses.

For a recognised world thumbnail, a world catalogue entry can supply the stored thumbnail key for that world. A legacy hexadecimal key with the same raw CIDv1 stays as projected.

## Consequences

The catalogue keeps the stored upstream URLs. Projected poster URLs point at the prefixed paths. The root `/poster` and `/poster-vertical` routes continue to serve operators that configure a dedicated `events-assets-099ac00.<domain>` vhost.

The node's existing asset handlers determine whether each asset is available. The path mapping does not copy world thumbnails into storage.
