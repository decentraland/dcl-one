# Pose emotes

The poses `wearable-preview` hosts ask for by name (`emote=fashion`, `love`, `walk`, ...) that are
not emotes on the catalyst. Copied unchanged from `decentraland/wearable-preview` at `cffe9ab`
(`public/emotes/`), as `apps/ui3/src/wearable-preview/emotes/` copies its own set from that
repository. `web/preview.js` registers each with the engine on first use, under
`urn:decentraland:off-chain:base-emotes:preview-<name>` (`preview-<name>-loop` when it loops).

As the Unity renderer does with its default emote, a pose plays once in the marketplace and then
the avatar idles; in the builder it loops. A catalyst base emote named as the default follows the
same rule: when its own metadata disagrees (`dance`, `money` and `clap` loop), it is registered
again under a preview urn with the catalyst's files.
