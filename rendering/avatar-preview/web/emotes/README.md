# Pose emotes

The poses `wearable-preview` hosts ask for by name (`emote=fashion`, `love`, `walk`, ...) that are
not emotes on the catalyst. Copied unchanged from `decentraland/wearable-preview` at `cffe9ab`
(`public/emotes/`), as `apps/ui3/src/wearable-preview/emotes/` copies its own set from that
repository. `web/preview.js` registers each with the engine on first use, under
`urn:decentraland:off-chain:base-emotes:preview-<name>`.

| File | Loops |
| --- | --- |
| `fashion.glb`, `fashion-2.glb`, `fashion-3.glb`, `fashion-4.glb` | yes |
| `walk.glb`, `run.glb` | yes |
| `love.glb`, `jump.glb` | no |
