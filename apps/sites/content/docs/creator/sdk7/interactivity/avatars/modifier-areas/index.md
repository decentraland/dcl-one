> Hide avatars, hide nametags, or disable passports for players inside a region of your scene

# Avatar Modifier Areas

Avatars behave and look consistently throughout Decentraland as they walk across scenes. However, you can add an `AvatarModifierArea` to a region of your scene to affect how player avatars behave when they enter that area.


> [!CAUTION]
> **❗Warning**\
> Please limit the amount of `AvatarModifierAreas` you use in your scene to just a couple. If you use too many of them, it may have a significant impact on performance.


### Placing Avatar Modifier Areas

Add an entity with an `AvatarModifierArea` component and position this entity by using a `Transform` component.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

When creating an `AvatarModifierArea` component, you must provide the following:

* `area`: Size of the modifier area
* `modifiers`: An array listing the modifiers to implement in the area. This property uses values from the `AvatarModifierType` enum.

The supported modifiers are:

* `AvatarModifierType.AMT_HIDE_AVATARS`
* `AvatarModifierType.AMT_DISABLE_PASSPORTS`
* `AvatarModifierType.AMT_HIDE_NAMETAGS`

All the effects of an `AvatarModifierArea` only take place within the region of their area. Players return to normal when they walk out of the area.

An `AvatarModifierArea` affects only players that are inside the area. Entering the area doesn't affect how other players outside the area are perceived.

The effects of an `AvatarModifierArea` are calculated locally for each player. You can have an `AvatarModifierArea` that is only present in the scene for some players and not for others. For example, you could make a "marco polo" game, where only one player in the scene has a modifier area that hides all of the other players. All the other players that don't have this modifier area in their local version of the scene are able to see each other normally.

If the area hides avatars, then the players that don't have the area in their local version of the scene will see all avatars normally, even those that experience themselves as hidden. Players that do have the area will experience themselves and all other avatars as affected by the area when they enter it.


> [!WARNING]
> **📔 Note**: Avatar modifier areas are affected by the _position_ and _rotation_ of the Transform component of their host entity, but they're not affected by the _scale_.



> [!WARNING]
> **📔 Note**: The `AvatarModifierArea`component must be imported via
> 
> > `import { AvatarModifierArea } from "@dcl/sdk/ecs"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


### Hide avatars

When a player walks into an `AvatarModifierArea` that has the `AvatarModifierType.AMT_HIDE_AVATARS` modifier, the player's avatar stops being rendered. This applies both for the player in 3rd person view, and for when other players walk into the area.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

This allows you to replace the default Decentraland avatar with any custom avatar you might want to show in your scene, see [Replace the player's avatar](/docs/creator/sdk7/interactivity/avatars/npc-avatars/#replace-the-players-avatar). Note that if you want to see other players with custom avatars, you should handle the syncing of player positions yourself.

### Disable Passport Popup

When a player walks into an `AvatarModifierArea` that has the `AvatarModifierType.AMT_DISABLE_PASSPORTS` modifier, clicking on them no longer opens up the passport UI that shows the player bio, inventory, etc.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

This is especially useful in games where accidentally opening this UI could interrupt the flow of a game, for example in a multiplayer shooter game.

### Hide nametags

When a player walks into an `AvatarModifierArea` that has the `AvatarModifierType.AMT_HIDE_NAMETAGS` modifier, the player's nametag is hidden while the avatar itself remains visible.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

This is useful for stages, presentations, or scripted scenes where you want to suppress player nametags without hiding the avatars themselves. For example, you might want a clean visual experience during a performance, where avatars are visible but the floating names don't distract viewers.

`AMT_HIDE_NAMETAGS` also hides any custom plates added with the [`AvatarNametag`](/docs/creator/sdk7/interactivity/avatars/avatar-nametags/) component.


> [!NOTE]
> **💡 Tip**: `AMT_HIDE_AVATARS` already hides nametags along with the avatar, so you don't need to add `AMT_HIDE_NAMETAGS` when using `AMT_HIDE_AVATARS`. Use `AMT_HIDE_NAMETAGS` only when you want to hide nametags while keeping avatars visible.


You can combine `AMT_HIDE_NAMETAGS` with other modifiers like `AMT_DISABLE_PASSPORTS` on the same area:

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!NOTE]
> **💡 Tip**: The nametag is only hidden while the player's head or torso is inside the area. If the area is too short and the player double-jumps above it, the nametag will briefly reappear. Make the area tall enough to cover the expected range of movement.


### Exclude Avatars

You can exclude a list of players from being affected by a modifier area by adding their player Ids to an array in the `excludeIds` property of the modifier area.

This example hides all avatars in an area, except those of players with specific IDs. You could use this for example on a live event, to only show the event hosts on the stage, and hide any other players that jump onto the stage.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: ['0xx1...', '0xx2...'],
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!WARNING]
> **📔 Note**: Make sure the player IDs are all written with lower-case letters. Use `.toLowerCase()` if necessary.


Modifier areas run locally on each player's instance. The list of excluded IDs can be different for each player. In the example below, each player excludes their own ID from a modifier that hides avatars, so that they each view their own avatar and no others.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	let userData = getPlayer()
	if (!userData) return

	const entity = engine.addEntity()

	AvatarModifierArea.create(entity, {
		area: Vector3.create(16, 5, 16),
		modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
		excludeIds: [userData.userId],
	})

	Transform.create(entity, {
		position: Vector3.create(8, 0, 8),
	})
}
```


> [!CAUTION]
> **❗Warning**\
> If the list of excluded IDs is going to be periodically changed (for example based on players entering or leaving an area), make sure that the list is kept in order. Perform a `.sort()` on the array, so that the list remains in the same order each time it's passed. This way, only the changes to the list are computed. Otherwise, this can have a significant impact on the scene's performance.
> 
> ```ts
> AvatarModifierArea.create(entity, {
> 	area: Vector3.create(16, 5, 16),
> 	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
> 	excludeIds: myAvatarList.sort(),
> })
> ```


### Debug modifier areas

It can be tough to know exactly what parts of the scene your modifier areas cover based on the code. Visual feedback helps a lot to confirm that they're well placed.

To verify the positions of a `AvatarModifierArea` or a `CameraModeArea`, give the entity holding it a `MeshRenderer` component with a `box` shape, and set the scale to the same size as the `area` of the modifier area.


> [!WARNING]
> **📔 Note**: Modifier areas aren't affected by the `scale` property of the transform, their size is based on their `area` property.


```ts
const entity = engine.addEntity()
const areaSize = Vector3.create(8, 3, 8)

AvatarModifierArea.create(entity, {
	area: areaSize,
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
  	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
	scale: areaSize,
})

MeshRenderer.setBox(entity)
Material.setPbrMaterial(entity, {
	albedoColor: Color4.create(0.5, 0.5, 0.5, 0.5),
})
```

To activate the effects of the modifier area, the player's head or torso must enter the area. It won't take effect if only the player's feet are in the area. Make sure the player can't easily evade the area by jumping.


> [!WARNING]
> **📔 Note**: The full area should fit inside the limits of your scene.



> [!NOTE]
> **💡 Tip**: For working examples, see the [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas) test scene, which combines `AvatarModifierArea` with runtime-mutated `excludeIds`, and [`10,99-avatar-modifier-hide-nametags`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/10,99-avatar-modifier-hide-nametags), which isolates the `AMT_HIDE_NAMETAGS` modifier.

