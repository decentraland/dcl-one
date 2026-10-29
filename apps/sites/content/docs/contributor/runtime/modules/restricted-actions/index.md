# restricted-actions

The `RestrictedActions` module allows scenes to access sensitive (and thus restricted) functionality. It's linked to the [`permission`](/docs/contributor/content/entity-types/scenes/#permissions) system, which scenes use to request the use of individual methods.

```ts
const RestrictedActions = require("~system/RestrictedActions");
```


> [!NOTE]
> As a World Explorer implementer, your runtime could have no enforcement of permissions. We **strongly** advise against this, since it puts players in danger.


Most of the restricted functionality is provided by this module, but there's also restricted [global functions](/docs/contributor/runtime/globals/).

The module contains the following methods and types:

* [`function movePlayerTo`](/docs/contributor/runtime/modules/restricted-actions/#movePlayerTo)
* [`function teleportTo`](/docs/contributor/runtime/modules/restricted-actions/#teleportTo)
* [`function triggerEmote`](/docs/contributor/runtime/modules/restricted-actions/#triggerEmote)
* [`function changeRealm`](/docs/contributor/runtime/modules/restricted-actions/#changeRealm)
* [`function openExternalUrl`](/docs/contributor/runtime/modules/restricted-actions/#openExternalUrl)
* [`function openNftDialog`](/docs/contributor/runtime/modules/restricted-actions/#openNftDialog)
* [`function setCommunicationsAdapter`](/docs/contributor/runtime/modules/restricted-actions/#setCommunicationsAdapter)
* [`interface Vector3`](/docs/contributor/runtime/modules/restricted-actions/#Vector3)

### Methods

Each of the methods below is associated with a specific [permission](/docs/contributor/content/entity-types/scenes/#permissions) that can be requested in the scene manifest.

**`movePlayerTo`**

Displace the player to a new position relative to the current one, and optionally set the camera target with [vectors](/docs/contributor/runtime/modules/restricted-actions/#Vector3).

Requires the [`ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE`](/docs/contributor/content/entity-types/scenes/#permissions) permission.

```ts
interface Request {
  newRelativePosition: Vector3;
  cameraTarget?: Vector3;
}

interface Response {
    // Once the player has moved to the new position, and if the player didn't interrupt the movement, this will be true.
  success: boolean;
}

function movePlayerTo(Request): Promise<Response>;
```

**`teleportTo`**

Reposition the player to an absolute world location given a by [vectors](/docs/contributor/runtime/modules/restricted-actions/#Vector3).

Instead of requiring a pre-approved permission, each call to `teleportTo` must be approved by the player.

```ts
interface Request {
  worldPosition: Vector3;
}

interface Response {}

function teleportTo(Request): Promise<Response>;
```

**`triggerEmote`**

Make the player's avatar display an emote animation, using one of the predefined names.

Requires the [`ALLOW_TO_TRIGGER_AVATAR_EMOTE`](/docs/contributor/content/entity-types/scenes/#permissions) permission.

```ts
interface Request {
  predefinedEmote: string;
}

interface Response {}

function triggerEmote(Request): Promise<Response>;
```

**`openExternalUrl`**

Offer to show a website to the player, using an appropriate UI (which may be another application).

Requires the [`OPEN_EXTERNAL_LINK`](/docs/contributor/content/entity-types/scenes/#permissions) permission.

```ts
interface Request {
  url: string;
}

interface Response {
  // Whether the player authorized opening the link.
  success: boolean;
}

function openExternalUrl(Request): Promise<Response>;
```

**`openNftDialog`**

Show information about an NFT to the player, using an appropriate UI.

```ts
interface Request {
  // The URN of the NFT.
  urn: string;
}

interface Response {
  // Whether the NFT was successfully located and referred to an image or video.
  success: boolean;
}

function openNftDialog(Request): Promise<Response>;
```

**`changeRealm`**

Switch the World Explorer to another content server, using its base URL.

```ts
interface Request {
  // The URL of the new realm.
  realm: string;

  // An optional message to show users when they have to approve the change.
  message?: string;
}

interface Response {
  // Whether the realm change was authorized and successful.
  success: boolean;
}

function changeRealm(Request): Promise<Response>;
```

### Types

The only additional type used by methods in this module is the `Vector3`.

**`Vector3`**

Holds a relative or absolute 3d position.

```ts
interface Vector3 {
  x: number;
  y: number;
  z: number;
}
```
