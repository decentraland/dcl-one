> Link to other scenes or external sites

# External Links

You can add links from your scene out to other content, either to other scenes or to external websites.

## Use the Scene Editor in Creator Hub

The easiest way to add an external link or a teleport is to use the Scene Editor. Use the **Teleport** [Smart Item](/docs/creator/scene-editor/interactivity/smart-items/) to add teleport to another scene in Genesis City, or use any of the **Social Links** smart items to add links to external sites.

![](/docs/creator/images/editor/social-links.webp)

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

To send a player to a scene that is not published in the open world Genesis City map, but instead to an isolated [Decentraland WORLD](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), use the function `changeRealm()`.

```ts
import { changeRealm } from "~system/RestrictedActions"

(...)

changeRealm({realm: 'mannakia.dcl.eth'})
```

Players are presented a confirmation screen before they are teleported, this screen displays information from the destination scene’s `scene.json file`, including the scene `name`, `description` and `navmapThumbnail`. See [scene metadata](/docs/creator/sdk7/projects/scene-metadata/) for details on how to set this data.

The player will spawn in one of the spawn points of the scene in that world, regardless of their current coordinates on the map.

To send a player back to Genesis City from a world, use `changeRealm` setting the `realm` field to _'https://realm-provider-ea.decentraland.org/main'_.


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
