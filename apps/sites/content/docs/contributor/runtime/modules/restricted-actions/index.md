# restricted-actions

The `RestrictedActions` module allows scenes to access sensitive (and thus restricted) functionality. It's linked to the [`permission`](/docs/contributor/content/entity-types/scenes/#permissions) system, which scenes use to request the use of individual methods.

```ts
const RestrictedActions = require("~system/RestrictedActions");
```


> [!NOTE]
> As a World Explorer implementer, your runtime could have no enforcement of permissions. We **strongly** advise against this, since it puts players in danger.


Most of the restricted functionality is provided by this module, but there's also restricted [global functions](/docs/contributor/runtime/globals/).

The module contains the following methods and types:

* [`function movePlayerTo`](/docs/contributor/runtime/modules/restricted-actions/#moveplayerto)
* [`function teleportTo`](/docs/contributor/runtime/modules/restricted-actions/#teleportto)
* [`function triggerEmote`](/docs/contributor/runtime/modules/restricted-actions/#triggeremote)
* [`function changeRealm`](/docs/contributor/runtime/modules/restricted-actions/#changerealm)
* [`function openExternalUrl`](/docs/contributor/runtime/modules/restricted-actions/#openexternalurl)
* [`function openNftDialog`](/docs/contributor/runtime/modules/restricted-actions/#opennftdialog)
* [`function openExplorerUi`](/docs/contributor/runtime/modules/restricted-actions/#openexplorerui)
* [`function stopEmote`](/docs/contributor/runtime/modules/restricted-actions/#stopemote)
* [`function triggerSceneEmote`](/docs/contributor/runtime/modules/restricted-actions/#triggersceneemote)
* [`function copyToClipboard`](/docs/contributor/runtime/modules/restricted-actions/#copytoclipboard)
* [`function setCommunicationsAdapter`](/docs/contributor/runtime/modules/restricted-actions/#setcommunicationsadapter)
* [`interface Vector3`](/docs/contributor/runtime/modules/restricted-actions/#vector3)

### Methods

Each of the methods below is associated with a specific [permission](/docs/contributor/content/entity-types/scenes/#permissions) that can be requested in the scene manifest.

### `movePlayerTo` {#moveplayerto}

Displace the player to a new position relative to the current one, and optionally set the camera target with [vectors](/docs/contributor/runtime/modules/restricted-actions/#vector3).

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

### `teleportTo` {#teleportto}

Reposition the player to a parcel, optionally in another realm.

Instead of requiring a pre-approved permission, each call to `teleportTo` must be approved by the player.

```ts
interface Request {
  // The parcel to land on. Omitted: the realm's default spawn
  // (only meaningful together with `realm`).
  worldCoordinates?: Vector2;

  // The realm the parcel belongs to: a world name (`foo.dcl.eth`) or a realm URL.
  // When set, the client changes realm (a full reconnect, even to the realm the
  // player is already in) and then lands on the parcel there.
  // Omitted: the parcel is in the player's current realm.
  realm?: string;
}

interface Response {}

function teleportTo(Request): Promise<Response>;
```

### `triggerEmote` {#triggeremote}

Make the player's avatar display an emote animation, using one of the predefined names.

Requires the [`ALLOW_TO_TRIGGER_AVATAR_EMOTE`](/docs/contributor/content/entity-types/scenes/#permissions) permission.

```ts
interface Request {
  predefinedEmote: string;
}

interface Response {}

function triggerEmote(Request): Promise<Response>;
```

### `openExternalUrl` {#openexternalurl}

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

### `openNftDialog` {#opennftdialog}

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

### `openExplorerUi` {#openexplorerui}

Open an explorer panel, fullscreen or modal, and return the open verdict.

```ts
interface Request {
  ui: ExplorerUi;

  // Echoed back in the events this request produces, so they can be matched to
  // their call. Unset means no correlation, and 0 is reserved for the same
  // meaning on the result, so ids start at 1.
  requestId?: number;

  // Only for `EU_ITEM_PURCHASE`.
  itemPurchase?: { urn: string };
}

interface Response {
  // Carries only the verdict of the open action.
  openResult: OpenExplorerUiResult;
}

function openExplorerUi(Request): Promise<Response>;
```

### `stopEmote` {#stopemote}

Stop the emote the player's avatar is currently playing.

```ts
interface Request {}

interface Response {
  success: boolean;
}

function stopEmote(Request): Promise<Response>;
```

### `triggerSceneEmote` {#triggersceneemote}

Make the player's avatar display an emote animation from a file in the scene.

```ts
interface Request {
  // The path to the emote file within the scene.
  src: string;

  loop?: boolean;
  mask?: AvatarMask;
}

interface Response {
  success: boolean;
}

function triggerSceneEmote(Request): Promise<Response>;
```

### `copyToClipboard` {#copytoclipboard}

Copy the provided text into the player's clipboard.

```ts
interface Request {
  text: string;
}

interface Response {}

function copyToClipboard(Request): Promise<Response>;
```

### `setCommunicationsAdapter` {#setcommunicationsadapter}

Ask the World Explorer to connect to another communications adapter, which scenes use to join private game servers.

```ts
interface Request {
  connectionString: string;
}

interface Response {
  success: boolean;
}

function setCommunicationsAdapter(Request): Promise<Response>;
```

### `changeRealm` (deprecated) {#changerealm}

Switch the World Explorer to another content server, using its base URL.

Deprecated in favour of [`teleportTo`](/docs/contributor/runtime/modules/restricted-actions/#teleportto) with a `realm` (and no `worldCoordinates` for the realm's default spawn). `changeRealm` resolves once the change is accepted rather than once the new realm is live, so a `teleportTo` issued straight after it runs against the previous realm.

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

### `Vector3` {#vector3}

Holds a relative or absolute 3d position.

```ts
interface Vector3 {
  x: number;
  y: number;
  z: number;
}
```
