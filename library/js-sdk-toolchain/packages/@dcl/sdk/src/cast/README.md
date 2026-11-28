# @dcl/sdk/cast

Follows the scene's stream and Cast for a scene: whether a stream is live and from what kind of
source, what the video player reports, the presentation, the presenters and the stream key. The
state is one value (`State`), changed only by the pure reducer `step` in `machine.ts`; `runtime.ts`
runs the effects the reducer returns.

The handle's role decides what a scene can send. The server still decides what the wallet may do,
and a refusal arrives as a typed `state.error` that says if and when to retry.

| Role | Reads | Commands |
| --- | --- | --- |
| `viewer` | the room's tracks and the slide state (no request to the server) | none |
| `admin` | also the presenters while a stream is live | `promote`, `demote`, `slide`, `play`, `pause`, `stop` |
| `owner` | also the stream key | also `generate`, `reset`, `revoke`, `link`, `present` |

Budget for each connection: the tracks once a second and the slide packets four times a second
(both local to the explorer), each server read at most once a minute, one write at a time. A read
that fails is sent again no sooner than its period, then with backoff up to 5 min; `state.error`
says when. `onChange` runs when something a scene can show changes, not on each read.

## Screen shape

`cast.show(screen)` keeps the shape of the picture. The screen gets the largest size of `ratio(state)`
inside the width and height it has at that call (`Transform.scale`): the slide's size while a
presentation shows, else the stream's size where the explorer lists it (Unity does, the web Explorer
does not). While the size is not known, the screen has the scene's size. `cast.show(screen, false)`
leaves the size alone.

## Slide video

`state.show.slides.video` is `idle`, `loading`, `playing`, `paused` or `error`. An admin sends
`{ do: 'play', video }` and `{ do: 'pause' }`.

The slide state comes from the presentation's own messages. A viewer gets it at the next message
after it arrives (a slide change); an admin that is a presenter asks for it at once.

## Example

```ts
import { engine, InputAction, Material, MeshCollider, MeshRenderer, pointerEventsSystem, Transform } from '@dcl/sdk/ecs'
import { openCast } from '@dcl/sdk/cast'

export function main() {
  const screen = engine.addEntity()
  Transform.create(screen, { position: { x: 8, y: 3, z: 8 }, scale: { x: 8, y: 4.5, z: 1 } })
  MeshRenderer.setPlane(screen)
  MeshCollider.setPlane(screen)
  Material.setBasicMaterial(screen, { texture: Material.Texture.Video({ videoPlayerEntity: screen }) })

  // 'viewer' follows the stream and asks the server nothing; 'admin' adds Cast, 'owner' the stream key.
  const cast = openCast('owner')
  // The screen keeps the picture's shape inside 8 x 4.5.
  cast.show(screen)
  cast.onChange((state) => {
    if (state.tag === 'unavailable') return console.log(`no streaming in this realm: ${state.error.code}`)
    if (state.tag !== 'following') return
    const { stream, video, key, show, error } = state
    console.log(stream.tag === 'live' ? `live from a ${stream.source.kind} source, video ${video}` : 'no stream')
    if (show.tag === 'showing') console.log(`slide ${show.slides.slide + 1} of ${show.slides.count}`, show.slides.video)
    if (key.tag === 'expiring') console.log(`the stream key ends at ${new Date(key.key.endsAt).toISOString()}`)
    if (key.tag === 'replaced') console.log('another admin made a new stream key')
    if (error) console.log(`${error.of}: ${error.code}`, error.retry.tag === 'at' ? `again at ${error.retry.ms}` : '')
  })

  // A command is one typed value; the answer arrives as the next state, a refusal as `state.error`.
  pointerEventsSystem.onPointerDown(
    { entity: screen, opts: { button: InputAction.IA_PRIMARY, hoverText: 'Stream key' } },
    () => cast.send({ do: 'generate' })
  )
}
```
