> Display and control NPC avatar

# NPC Avatars

Display an avatar as an entity in a scene.


> [!NOTE]
> **💡 Tip**: Try the [NPC Toolkit library](https://github.com/decentraland-scenes/dcl-npc-toolkit) for an easier experience dealing with NPCs, especially if you need to interact via a conversation tree.


## Create an avatar

The following snippet creates an avatar with random wearables and body shape, and name "NPC".

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar)

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

When passing data to generate an `AvatarShape`, the following fields are required:

* `id`: (required) Internal identifier for the Avatar. The last 4 characters of the `id` show after the NPC's name, in faded text, like a player's wallet suffix. An `id` shorter than 4 characters shows `0000`. Give each NPC an `id` whose ending reads well next to its name, for example `'guide-0001'`.

The following optional fields are also available:

* `name`: Name to display over the Avatar's head. Default: "NPC". Set it to an empty string to show no name at all, which is useful when you only want an [Avatar Nametag](/docs/creator/sdk7/interactivity/avatars/avatar-nametags/) plate above the NPC.
* `bodyShape`: String to define which body shape to use. Valid options are 'urn:decentraland:off-chain:base-avatars:BaseMale' and 'urn:decentraland:off-chain:base-avatars:BaseFemale'.
* `wearables`: Array with list of URNs for wearables that the avatar currently has on. If wearables conflict (like two of them are hats), the last one in the list replaces the other.
* `emotes`: Array with list of URNs for NFT emotes that the avatar is capable of playing
* `eyeColor`: _Color3_ for the eye color (any color is valid)
* `skinColor`: _Color3_ for the skin color (any color is valid)
* `hairColor`: _Color3_ for the hair color (any color is valid)
* `talking`: If _true_, it displays a green set of bars next to the name, like when players use voice chat in-world.
* <div data-gb-custom-block data-tag="hint" data-style="info" class="hint hint-info"><p><strong>💡 Tip</strong>: See <a href="/docs/creator/sdk7/3d-essentials/color-types/">color types</a> for more details on how to set colors.</p></div>


> [!WARNING]
> **📔 Note**: The `AvatarShape`component must be imported via
>
> > `import { AvatarShape } from "@dcl/sdk/ecs"`
>
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.



> [!WARNING]
> **📔 Note**: The URN fields must follow the same format used for [NFTShapes](/docs/creator/sdk7/media/display-a-certified-nft/): `urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`


## Move an NPC

To move an NPC, change the `position` of its `Transform`. The avatar doesn't jump to the new position: it walks there from where it is, at about 4 meters per second, and moves faster when the target is 1.5 meters away or more. While it moves, it turns to face the direction it's walking in. The `rotation` you set in the `Transform` is only applied once it arrives.

```ts
// The NPC walks from its current position to the new one
Transform.getMutable(myAvatar).position = Vector3.create(10, 0.25, 5)
```

To control the movement yourself, use a [Tween](/docs/creator/sdk7/3d-essentials/move-entities/). A Tween on the position replaces this automatic walk, and a Tween on the rotation stops the NPC from turning to face where it walks. An entity can only have one Tween at a time, so you can't tween both the position and the rotation of the same entity at once.

## Animations

Avatars play default idle animations while still.

To play animations on the avatar, set the `expressionTriggerId` string to the name of the animation you want to play.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'dancer-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'robot',
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

The `expressionTriggerId` field supports all [default animations](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#default-animations), as well as custom animations [from a scene file](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#custom-animations), and even URNs from emotes that are published to the marketplace.

To use a custom animation, give the path to the `_emote.glb` file from your project's root folder, for example `'assets/animations/Cheer_emote.glb'`. As with [`triggerSceneEmote()`](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#custom-animations), a wrong path plays nothing.


> [!WARNING]
> **📔 Note**: `AvatarShape` has no `mask` field, so an `expressionTriggerId` animation always plays on the whole body. [Animating only the upper body](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#animate-only-the-upper-body) is available on the player's own avatar, not on NPCs.


### Moving an NPC while it plays an emote

Writing the avatar's `Transform.position`, or running a [Tween](/docs/creator/sdk7/3d-essentials/move-entities/) on it, cancels the emote that is playing. The avatar drops back to its walk cycle, because Decentraland works out which locomotion animation to blend from how far the avatar moved since the last frame.

To move an NPC during an emote, if the movement is over a small area you may want to bake the movement into the animation itself, as root or hips motion inside the `_emote.glb` file.

### Looping Animations

Animations on an `AvatarShape` play once, if you want the avatar to keep looping an animation, you should create a system that tells it to play the animation again every couple of seconds.

The one exception is the looping default emotes, `sittingChair1`, `sittingChair2`, `sittingGround1` and `sittingGround2`, which hold their pose on an `AvatarShape` with no need to replay them. A custom animation from a file never loops on an `AvatarShape`, even if the clip itself is marked as looping, so a sustained custom pose needs the replay pattern below.

Use the `expressionTriggerTimestamp` to replay a same emote. The value of this field is a [lamport timestamp](https://en.wikipedia.org/wiki/Lamport_timestamp), meaning that it's not a time value, but rather an index that is raised by 1 for each repetition of the emote.

So the first time you play an emote, you set `expressionTriggerTimestamp` to _0_. To play the emote again, you must update this value to 1. That's how the engine knows that this is a new instruction, and not an instruction it already acted upon.

The following snippet creates a system that runs a same emote every 2 seconds:

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'fan-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'clap',
    expressionTriggerTimestamp: 0
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})

let clapTimer = 0
let emoteDuration = 2  // 2 seconds

// system
engine.addSystem((dt: number) => {
    clapTimer += dt
      
    if (clapTimer >= emoteDuration) {
        // Trigger the clap emote
        AvatarShape.getMutable(myAvatar).expressionTriggerTimestamp += 1
        
        clapTimer = 0 // Reset timer
    }
})
```


> [!NOTE]
> **💡 Tip**: You must know the duration of the emote, and make that the duration of the system. If you create an emote that fixes the avatar still in a same pose, it's recommendable to make the duration of the emote longer than the system. That way, you can make sure that there are no artifacts when finishing and resetting the animation.


## Add a label above the name

Use the `AvatarNametag` component to show a plate with custom text above the NPC's name, for example a role like "Shopkeeper" or "Boss". Attach it to the same entity that has the `AvatarShape`:

```ts
AvatarNametag.create(myAvatar, { label: 'Shopkeeper' })
```

If the `AvatarShape` has an empty `name`, only the plate is shown, with no empty nametag box under it. This is handy for labeling an NPC with a title alone.


> [!NOTE]
> **💡 Tip**: For a color-coded plate with no text, set `label` to a string of spaces and pick a `backgroundColor`. Spaces are preserved, so more spaces make a wider plate.
>
> ```ts
> AvatarNametag.create(myAvatar, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```


See [Avatar Nametags](/docs/creator/sdk7/interactivity/avatars/avatar-nametags/) for the full list of fields, including colors.

## Copy wearables from player

The following snippet changes the wearables and other characteristics of an NPC avatar to match those that the player currently has on. This could be used in a scene as a manequin, to show off a particular wearable or emote combined with the player's current outfit.

```ts
import { getPlayer } from '@dcl/sdk/src/players'


export function swapAvatar(avatar: Entity) {

  let userData = getPlayer()
  console.log(userData)

  if (!userData || !userData.wearables) return

  const mutableAvatar = AvatarShape.getMutable(avatar)

  mutableAvatar.wearables = userData.wearables
  mutableAvatar.bodyShape = userData.avatar?.bodyShapeUrn
  mutableAvatar.eyeColor = userData.avatar?.eyesColor
  mutableAvatar.skinColor = userData.avatar?.skinColor
  mutableAvatar.hairColor = userData.avatar?.hairColor
  
}
```

## Display only wearables

Use the `showOnlyWearables` field to display only the listed wearables of an avatar. The rest of the avatar's body will be invisible.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'mannequin-0001',
	emotes: [],
	wearables: [
    'urn:decentraland:matic:collections-v2:0x90e5cb2d673699be8f28d339c818a0b60144c494:0'
  ],
	showOnlyWearables: true,
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

This is useful for displaying wearables, for example in a store.


> [!NOTE]
> **💡 Tip**: If a wearable is rather small, try setting the `scale` of the `Transform` to a larger value.


## Attach an entity to an NPC

You can use the `AvatarAttach` feature to fix an entity to one of the bones of an NPC avatar, for example so that the NPC is holding an object on their hand. The entity will move together with the avatar when it animates.

To use this feature, use the `id` property on the `AvatarShape` to assign an arbitrary id to this avatar, and then reference that id on the `AvatarAttach`. The id can be any string you want.

```ts
// Create NPC, with an ID
const myAvatar = engine.addEntity()
Transform.create(myAvatar, {
  position: Vector3.create(8, 0.25, 8),
})
AvatarShape.create(myAvatar, {
  id: "my-avatar-id", 
  wearables: [],
  emotes: []
})

// Create object to attach to NPC
const attachedEntity = engine.addEntity()
Transform.create(attachedEntity, {
    position: Vector3.create(4, 2, 4),
    scale: Vector3.create(0.15,0.15,0.15)
})
MeshRenderer.setBox(attachedEntity)
Material.setBasicMaterial(attachedEntity, { diffuseColor: Color4.Blue() })
AvatarAttach.create(attachedEntity, {
    avatarId: "my-avatar-id",
    anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND
})
```

Learn more about the `AvatarAttach` component [here](/docs/creator/sdk7/3d-essentials/entity-positioning/#attach-an-entity-to-an-avatar).

## Replace the player's avatar

You can't change what wearables the player's avatar is wearing, but you can instead swap the player's avatar for an NPC avatar that you can fully customize.

To do this, hide the player's own avatar with an [Avatar Modifier Area](/docs/creator/sdk7/interactivity/avatars/modifier-areas/#hide-avatars) that uses the `AMT_HIDE_AVATARS` modifier, and display an `AvatarShape` entity at the player's position instead.


> [!WARNING]
> **📔 Note**: To allow the player to have full control over that avatar, you should listen to button events to detect when they press a button, and then trigger the corresponding animation on the NPC avatar. See [Button Events](/docs/creator/sdk7/interactivity/button-events/system-based-events/) for more details.
>
> The fluidity of control may not be perfect while doing this, you may want to use this only in very specific cases.

