# Event posters for native players

- Status: Accepted
- Date: 2026-11-24

## Context

Installed Unity players request event artwork through the native conversion route. They can display JPEG and PNG on the first request. A WebP response can leave the calendar card, detail cover or community event thumbnail blank. Bevy desktop and web Play display event posters in the shared HTML HUD. Sites also requests explicit-width browser thumbnails. [Shared Explorer HUD (accepted)](2026-11-10-shared-explorer-hud-ui.md)

## Decision

For a recognized event poster, `GET /convert?url=…` and its gateway alias `/metamorph-api/convert` serve a final still image when the caller omits `width`. The first request and subsequent requests use the same representation rules.

| Source bytes | Native response |
| --- | --- |
| JPEG or PNG | Original bytes, dimensions and media type |
| WebP or GIF | First displayed frame, fitted within 960 × 960 pixels without enlargement and with its aspect ratio and orientation preserved. The renderer uses JPEG at quality 80 when all pixels are opaque and PNG when any pixel is translucent or transparent. |
| Other image types, including AVIF | HTTP 415 on the native poster request. The browser media route can still serve a supported original. |

Recognition uses the source URL as well as the request path. The canonical HTTPS bucket `events-assets-099ac00.decentraland.org` serves `/poster/<file>` and `/poster-vertical/<file>`. On the configured node, both paths sit under `PUBLIC_URL` plus `/events-assets-099ac00`. The node also serves the exact bundled poster at `PUBLIC_URL` plus `/events/assets/event-default.webp`. A deployment prefix in `PUBLIC_URL` is part of the path. Recognition requires a canonical URL and a valid poster filename, with no credentials, query or fragment. [Node asset URLs (accepted)](2026-11-18-node-asset-urls-by-path.md)

Browsers can fetch the original artwork, including animation, at the asset URL. The browser converter `/media/convert` and its gateway alias `/metamorph-api/media/convert` retain their media contract. An explicit supported `width` selects the existing thumbnail renderer on either route. Other source URLs and videos follow the generic media rules.

## Serving and caching

Catalyst enforces its existing source-access policy and checks redirects on remote fetches. Poster processing shares the thumbnail limits of 16 active source reads, 256 waiting requests and an 8 MiB source. The renderer accepts inputs up to 8192 pixels per dimension, applies a 128 MiB decoder allocation limit and caps generated output at 2 MiB. A full queue or a 20-second slot wait that expires returns 503 with `Retry-After: 1`. Errors carry `Cache-Control: no-store`.

Both native routes share one cache key per poster URL. Browser originals and width-specific derivatives have their own keys. Cacheable concurrent requests for the same native poster share a fetch. Successful responses carry `Cache-Control: public, max-age=86400`. The process cache holds up to 256 entries for five minutes, with a 2 MiB limit per entry. HEAD reports the GET status, representation length, media type and cache policy with an empty body. Previously cached originals can remain fresh at an edge or client for a day after rollout or rollback. [Gateway operator examples](../../infra/gateway/README.md#event-poster-conversion)

## Consequences and alternatives

A cache miss for WebP or GIF costs one bounded decode and encode on the node. The player decodes a raster and uploads its pixels. A supported block-compressed texture could use less GPU memory. Production Metamorph can publish a GPU-compressed KTX2 after an initial conversion request. This gateway deliberately serves its final raster on both cold and warm requests. An external conversion queue would add a pending state and a service dependency. Self-hosting that queue would also require encoder capacity and artifact storage. A player update could add WebP support to installations that receive it.

GPU-compressed textures have separate uses in 3D rendering. The Explorer has an accepted Basis Universal transcoder. The scene-upload and Bevy-variant proposals address textures sampled by the engine. Reopen this decision if a supported poster consumer requires KTX2 or comparable measurements on the same player, device, artwork dimensions and cache state show a material improvement in decode time or GPU memory. [Transcoder (accepted)](2026-11-04-vendored-basisu.md) · [Opt-in scene texture uploads (proposed)](2026-11-06-opt-in-uastc-texture-upload.md) · [Bevy variants (proposed)](2026-11-09-bevy-variants-manifest.md)
