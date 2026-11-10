# basic-entities

The World Explorer [reserves a range](/docs/contributor/runtime/modules/engine-api/#identifying) of entity IDs for its own use. Of this range, the identifiers `0`, `1` and `2` are pre-assigned to a set of basic entities guaranteed to exist.

These are:

- `0`: `RootEntity`
- `1`: `PlayerEntity`
- `2`: `CameraEntity`

These entities have a known set of attached components that are managed by the runtime, i.e. their state is read-only for the scene. Obtaining the initial state of these components is part of the [scene initialization](/docs/contributor/runtime/execution/#onstart).


> [!NOTE]
> Additional components for these entities are in development. The [CRDT synchronization mechanism](/docs/contributor/runtime/modules/engine-api/#synchronization) is expanding to provide some of the funcionality offered by different modules.
>
> For example, scenes will be able to detect the state of the Explorer's window in order to optimize their network and CPU usage, or access richer information about the player, the comms system and the game camera.


### `RootEntity` (`0`) {#rootentity}

Holds general information about the scene's context.

- [`Transform`](/docs/contributor/runtime/basic-components/#transform): positional information of the scene's origin point, to which all other transforms without an explicit parent are relative.

The `Transform` component of the `RootEntity` is initialized with the following values:

```yml
scale: [1, 1, 1] # XYZ
position: [0, 0, 0] # XYZ
rotation: [0, 0, 0, 1] # XYZW
```

### `PlayerEntity` (`1`) {#playerentity}

Holds data about the player, their identity, their avatar and location.

- [`Transform`](/docs/contributor/runtime/basic-components/#transform): the positional information for the player's avatar.

### `CameraEntity` (`2`) {#cameraentity}

Holds information about the game camera. All components are managed by the renderer.

- [`Transform`](/docs/contributor/runtime/basic-components/#transform): the positional information of the camera itself.
- [`PointerLocked`](/docs/contributor/runtime/basic-components/#pointerlock): whether the pointer follows the camera direction or moves independently.
