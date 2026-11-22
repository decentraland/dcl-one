# Stored files are served inert

- Status: Accepted
- Date: 2026-11-21

## Context

The content server and the worlds mount serve stored files at
`/contents/{hash}` and through the entity image and thumbnail routes. With
`?includeMimeType`, and always on the image and thumbnail routes, the
`Content-Type` comes from a port of `file-type` 21.1.1
(`services/catalyst/content/src/file_type/`).

The legacy catalyst
([`decentraland/catalyst@6c56f8a3`](https://github.com/decentraland/catalyst/tree/6c56f8a3eaff631cb7be0dc01ef92a24333bbc93),
`createContentFileHeaders` in `src/controllers/utils.ts`) answers whatever
`file-type` detects, `application/xml` included, and sends no
`Content-Security-Policy` or `X-Content-Type-Options` header.

Stored bytes are chosen by whoever deploys them. A browser opens a file served
with a document type (XML, HTML, text) as a page, on the origin that served it.
In dcl-one that origin is the node's, which also serves the admin console and
its session. The legacy content server's origin serves no authenticated
browser session.

## Decision

Stored files are served so that a browser never runs them as a page:

- `get_content::sniff` maps every document type to
  `application/octet-stream`: any `text/*`, `*/xml`, `*+xml` or HTML type. Of
  `file-type`'s answers that is `application/xml`, `text/vtt`, `text/vcard`,
  `text/calendar` and `application/vnd.ms-htmlhelp`. The detector itself still
  returns `file-type`'s strings, so it stays a faithful port.
- Every stored-file response carries `Content-Security-Policy: sandbox` and
  `X-Content-Type-Options: nosniff` (`formatters::content_file_headers`). The
  gateway copies the policy back onto files it serves through
  `X-Accel-Redirect` (`infra/gateway/gateway.py`).
- Every other detected type is answered as `file-type` answers it. That covers
  video, audio, images, glTF/GLB, fonts, archives and Office formats.

This is a deliberate, permanent difference from the legacy catalyst. Do not
restore the legacy answer for any document type, and do not drop the headers to
match legacy byte for byte. Parity work on these routes keeps this decision.

## Reasons

- Stored content must never run as a page on the node's origin. Mapping only
  `application/xml` would cover what `file-type` detects today, but a future
  `file-type` update that detects another document type would not be covered.
  The rule covers the whole family.
- No client needs a document type from these routes. The only known caller of
  `?includeMimeType` is the Unity explorer's video player, which needs video
  types. The image and thumbnail routes serve images. These answers are
  unchanged.
- The sandbox policy and `nosniff` cost nothing to `fetch`, `<img>`, `<video>`,
  `<audio>` or the explorer, which ignore them. They only affect a file opened
  directly as a page.

## Consequences

- `?includeMimeType` answers `application/octet-stream` where the legacy
  catalyst answers a document type.
  `services/catalyst/tests/fixtures/conformance/content-lambdas/content-reads.md`
  (CR-7) and `services/catalyst/docs/worlds-compatibility.md` record the
  difference.
- A stored file opened directly in a browser tab runs sandboxed. Browsers may
  not render a PDF that way.
- Moving the admin console to an origin of its own would not make this decision
  obsolete. Other node services share the origin, and the rule costs nothing.
