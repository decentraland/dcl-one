# ADR: Cast client composition is opt-in and keeps the slide track

- Status: Accepted
- Date: 2026-11-20

## Context

A Cast presentation reaches clients in two ways. In the first, the bot publishes the
slides as a video track and each client shows that track. In the second, client
composition, the bot's participant metadata names the current slide as a picture
(`slide`) and the presenter (`presenterIdentity`), and each client draws the slide, the
presenter's camera and the camera layout itself. Upstream
(`decentraland/cast-presenter-server` @ cae78612) selects the second with a feature flag
and a `PUBLIC_BASE_URL`, and then publishes no slide track.

The clients of this node differ. The Unity Explorer composes when the metadata has a
`slide`. It fetches the picture only from `https://cast-presenter-service.<its base
domain>`, and it draws a black slide when that fetch fails. The Cast pages, the Admin
Toolkit and the Bevy Explorer do not compose: they show the bot's screen-share track.

## Decision

- **Opt-in by address:** `CAST_PRESENTER_PUBLIC_URL` names the presenter's routes as
  clients reach them. With it, each session keeps the PNG of its last 8 slides, serves
  them at `GET /cast-presenter-service/presentations/{id}/slides/{hash}.png`, and its
  state carries `slide`, `presenterIdentity` and the video position fields. Unset, the
  state has none of them and the route answers 404.
- **A slide is named only when it is served:** the session stores a slide's picture
  before the state that names it, in the same step that changes the slide. The state has
  no composition field without a stored picture.
- **The slide track stays:** the bot publishes the `presentation` SCREENSHARE track in
  both modes, and no other video track. A composing client does not read it.
- **The presenter:** `presenterIdentity` comes from `POST /presentations`, then from the
  last `presentation:presenter:claim` of one of the room's presenters.
- **The picture route has no authorization:** the session id and the content hash are
  the capability, as upstream. The answer is `Cache-Control: private`. The route reads
  the session's memory only.

## Rejected options

- **Compose always, from `PUBLIC_URL`:** the node does not know the host a client derives
  for the presenter. A wrong address gives the Unity Explorer a black slide where it had
  the slide track.
- **Drop the slide track when composing, as upstream:** the Cast pages, the Admin Toolkit
  and the Bevy Explorer would show nothing.
- **Publish the `presentation-video` track now:** it carries embedded video, which this
  node does not play. A second screen share of the bot could be chosen in place of the
  slides by a client that takes the first one.

## Consequences

- The operator sets `CAST_PRESENTER_PUBLIC_URL` to
  `https://cast-presenter-service.<base domain>`, and the front sends that host to the
  node's `/cast-presenter-service`.
- Open gap: the picture route reads the session memory of one `serve` process, and the
  gateway spreads API requests over the replicas. Client composition needs one API
  replica, or `/cast-presenter-service` routed to one. With more replicas, most slide
  requests answer `404` and the Unity Explorer draws a black slide.
- A session holds at most 8 PNG of at most 1920x1080 pixels; the sessions are bounded by
  `MAX_CONCURRENT_PRESENTATIONS`.
- The camera appears in a composing client only after a caller names the presenter. The
  Cast streamer page of this tree sends the claim when the bot shows a slide whose
  presenter is not in the room (`apps/sites/src/features/social/cast-live/presenter.ts`).
  No client of this tree sends `presenterIdentity`, and the Admin Toolkit sends neither.
- The Unity Explorer accepts a `stream:` identity of at most 128 characters. This node's
  streamer identities are `stream:<place>:<access id>:<uuid>`, so a place name longer
  than 47 characters gives a slide without the camera.
- Embedded video stays unserved: the video fields are `null`, `slideVideos` entries have
  no `source`, and there is no `presentation-video` track.
