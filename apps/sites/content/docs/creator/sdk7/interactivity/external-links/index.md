> Link to other scenes or external sites

# External Links

You can add links from your scene out to other content, either to other scenes or to external websites.

## Use the Scene Editor in Creator Hub

The easiest way to add an external link or a teleport is to use the Scene Editor. Use the **Teleport** [Smart Item](/docs/creator/scene-editor/interactivity/smart-items/) to add teleport to another scene in Genesis City, or use any of the **Social Links** smart items to add links to external sites.

![](/docs/creator/images/editor/social-links.webp)

## Teleports

To teleport a player to another scene, call the following function, indicating the coordinates that you want players to teleport to.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ worldCoordinates: { x: -51, y: 1 } })
```

Players are presented a confirmation screen before they are teleported, this screen displays information from the destination scene’s `scene.json file`, including the scene `name`, `description` and `navmapThumbnail`. See [scene metadata](/docs/creator/sdk7/projects/scene-metadata/) for details on how to set this data.

Bear in mind that teleports take you to a scene at the indicated coordinates, but not necessarily to those same coordinates. This means that when travelling to a scene that has multiple parcels, players may not be landing on the same coordinates as specified, but rather into one of the spawn points designated by the creator of the scene.

To move a player to another set of coordinates inside the current scene, use the `movePlayerTo()` function instead. See [Move a Player](/docs/creator/sdk7/interactivity/avatars/move-player/).

## Teleport to a WORLD

To send a player to a scene that is not published on the open Genesis City map, but instead to an isolated [Decentraland WORLD](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), pass a `realm` to `teleportTo()`.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ realm: 'mannakia.dcl.eth' })
```

`realm` accepts either a world name such as `mannakia.dcl.eth`, or a realm URL.

With no `worldCoordinates`, the player lands on one of the destination's spawn points. Add coordinates to pick a specific parcel inside that world:

```ts
// Land on parcel 12,34 of a specific world
teleportTo({
	realm: 'mannakia.dcl.eth',
	worldCoordinates: { x: 12, y: 34 },
})
```

Players are presented a confirmation screen before they change realm. It asks _"Are you sure you want to enter this World?"_ and shows the destination (the world name, or the host of the realm URL). The player is only moved if they accept.

To send a player back to Genesis City from a world, don't include the `realm` field, which defaults to Genesis City.


> [!WARNING]
> **📔 Note**: The `realm` field requires `@dcl/sdk` version 7.28.0 or newer.


### changeRealm is deprecated

Older scenes used `changeRealm()` to reach a world. It still works, but use `teleportTo({ realm })` instead. `changeRealm()` also shows a confirmation screen; its optional `message` field only replaces the default prompt text.

`changeRealm()` resolves as soon as the player accepts the prompt, not when the new realm is actually live. A `teleportTo()` call right after it would run against the **old** realm and land the player in the wrong place. `teleportTo({ realm })` does both steps in one call, so that gap doesn't exist.

## External links

To add a link to an external website, use the `openExternalUrl()` command.

```ts
import { openExternalUrl } from '~system/RestrictedActions'

openExternalUrl({ url: 'google.com' })
```

To prevent any abusive usage of this feature to spam players, it's only possible to call the `openExternalUrl` from an explicit click or button event on an entity. It's not possible to call this function as a result of a timer, or a collision area, or a global click event. See [Button events](/docs/creator/sdk7/interactivity/button-events/click-events/) for details on how to do this.

When `openExternalUrl` is called, players are prompted with a confirmation screen, where they are informed of where the link will take them, and where can accept of decline to visit the link.

The link is opened in a new tab, keeping the original tab in Decentraland.

If players tick the _trust this domain_ checkbox, they won't be prompted again during their session, as long as the link comes from the same scene and is to the same domain.

## Copy to clipboard

To copy a string to the player's clipboard, use `copyToClipboard()`. After this, when the player does _paste_ in the Decentraland chat or in any other application on their machine, they will be pasting your string.

```ts
import { copyToClipboard } from "~system/RestrictedActions"

copyToClipboard( { text: 'My text to copy' } )
```
