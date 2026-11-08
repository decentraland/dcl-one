# ADR: the UUAV helper fetches http(s) media itself

- Status: Accepted
- Date: 2026-11-08
- Amends: [UUAV, the native video player](2026-11-03-uuav-native-video.md),
  [the UUAV runtime set is built with Nix](2026-11-08-uuav-nix-built-runtime.md)

## Context

FFmpeg's http protocol needs a TLS backend: mbedTLS on Linux and macOS
(Apache-2.0, which forces `--enable-version3` and LGPL-3.0 on the shared
libraries) and Schannel on Windows. The helper set none of FFmpeg's
verification options and FFmpeg 8 verifies nothing by default (`tls_verify`
is 0), so streams were fetched without checking the server's certificate,
through a different stack on each platform. HLS playlists, keys and AES-128
segments travel the same path.

## Decision

- FFmpeg is built without the `https`, `tls`, `crypto` and `httpproxy`
  protocols and without a TLS library; `tcp`, `udp` and `rtp` remain for
  the `rtsp` and `sdp` demuxers. The `http` protocol stays linked because
  the `rtsp` demuxer selects it (HTTP tunnelling, `rtsp_transport=http`,
  plain http under the whitelist); no `http:` url reaches it, since the
  helper serves them before FFmpeg resolves a protocol, and a unit test
  pins the protocol list.
- `uuav-core` serves `http:` and `https:` urls (`src/playback/http.rs`):
  ureq over rustls. The top-level stream is opened before
  `avformat_open_input` and handed over as a caller-owned `AVIOContext`;
  every nested url (HLS variant playlists, keys, segments, `crypto+`
  AES-128 segments with the demuxer's `key`, `iv`, `offset` and
  `end_offset` options) is intercepted in the format context's `io_open`
  and `io_close2`. Seekability follows FFmpeg's http protocol: a resource
  is seekable once the server advertised byte ranges (`Accept-Ranges:
  bytes`) or served one (206); otherwise the demuxer reads in order, and a
  backward seek still tries a Range request, skipping up to 1 MiB when the
  server ignores it and failing, logged, beyond that. A dropped connection
  is resumed at its position: at once, then after 0.5 s and after 2 s, and
  the failure after that is final; a byte read starts the count again.
  Cancelling a player ends the wait and shuts the client down, which
  unblocks a stalled request.
  `infra/packages/uuav-ffmpeg-hls-https.patch` lets the HLS demuxer
  name `https` urls while no https protocol is registered.
- Certificates are verified against the OS trust store
  (`rustls-native-certs`; on Windows the ROOT store opened read-only through
  `schannel`, because the helper already runs under its low-integrity token
  when it reads it and a store opened for writing is denied), read once by
  the helper before it sandboxes itself (`uuav_core::trust`). A store that
  yields roots is the policy: a root that the user or an administrator
  removed from it is not trusted. Mozilla's bundle (`webpki-root-certs`)
  stands in only when the store yields no root (a container without
  `ca-certificates`). The rule is the same on Windows, whose ROOT store
  holds only the roots Windows has fetched so far: a server under a root not
  fetched yet fails to verify. The certificates of the Windows Disallowed
  store are dropped from the result. The bundle changes
  only with the crate version; the macOS profile no longer opens the
  keychains, the MDS databases or the trustd brokers. A server that does not
  verify fails to open.
- The protocol whitelist keeps its meaning: `http` and `https` gate the
  helper's client, the other names gate FFmpeg's protocols. The two names
  gate every hop of a redirect too: without `http` on the list, an `https`
  url that redirects to `http` fails before a plain request leaves.
  `https,http,tcp,udp,rtp,data` is the baseline, `file` for local playback.

## Consequences

- One client, one verifier and one trust policy on the three platforms; the
  FFmpeg libraries are LGPL-2.1 everywhere and carry no TLS code.
- A root installed in the OS store (corporate interception) is honoured; a
  self-signed server is refused where FFmpeg 8 accepted it.
- Encrypted HLS from a `file:` playlist (`crypto:` segments without a
  scheme) is not served; local playback uses unencrypted media.
- FFmpeg's http options (`headers`, `user_agent`, cookies, `http_proxy`) are
  not consumed: the client sends a fixed `User-Agent` and follows up to
  eight redirects.
- A proxy comes from the helper's environment, read when the helper starts:
  `ALL_PROXY`, `HTTPS_PROXY` or `HTTP_PROXY` (either case), with `NO_PROXY`
  for exceptions. FFmpeg's http protocol read `http_proxy` and `no_proxy`
  only. On Linux the client starts the helper with a cleared environment
  (`uuav-client/src/spawn.rs`), so no proxy variable reaches it, as before.
  A proxy configured only in the system settings of Windows or macOS is
  used by neither.
