> Create wearables with interactive capabilities

# Smart Wearables

Smart wearables are a type of global scene. Like [portable experiences](/docs/creator/sdk7/projects/portable-experiences/), they are gameplay that players take with them as they move through the metaverse. For example, while running a global scene, a player could take a snowball from the ground in Genesis Plaza, walk away to another scene, and throw the snowball to another player who’s also playing the same game.

Smart wearables are portable experiences that are turned on when the player puts on a certain item of clothing. Smart wearables can grant players new abilities, like a jetpack that lets them fly, or add a new layer of content on top of the rest of the world, like randomly placing coins to be collected throughout the whole of genesis city.


> [!WARNING]
> **📔 Note**: Smart Wearables can only be created using SDK 7.
>
> The **Creator Hub** doesn't currently support creating Smart Wearables projects.


## Getting started

### Using the CLI

1. Open a command line in a new folder and run

`npx @dcl/sdk-commands init --project smart-wearable`

This command creates the basic files and structure for a new smart wearable.

## The files in the template

The folder of a brand new Smart Wearable project is very similar to that of a [Decentraland scene](/docs/creator/sdk7/projects/scene-files/), but you will notice the following differences:

- `wearable.json` includes all of the metadata for the portable experience
- There’s a placeholder 3D model (glasses.glb) and thumbnail (glasses.png) for a pair of dark glasses. You should replace these with the actual wearable you are creating
- `scene.json` includes `"isPortableExperience": true`. Parcels aren't relevant to the published wearable, which works anywhere, but you can set them for local testing: the preview only runs on the parcels listed in `scene.json`

## About wearable.json

The default `wearable.json` file looks like this:

```json
{
	"data": {
		"replaces": [],
		"hides": [],
		"tags": ["special", "new", "eyebrows"],
		"representations": [
			{
				"bodyShapes": [
					"urn:decentraland:off-chain:base-avatars:BaseMale",
					"urn:decentraland:off-chain:base-avatars:BaseFemale"
				],
				"mainFile": "glasses.glb",
				"contents": ["glasses.glb"],
				"overrideHides": [],
				"overrideReplaces": []
			}
		],
		"category": "eyewear"
	},
	"name": "Smart Wearable Example",
	"description": "Put the glasses to see a new world",
	"rarity": "mythic"
}
```

These are the most common fields in `wearable.json`:

- `id`: Unique id of the smart wearable.
- `name` (required): The name for the wearable that users will see in the marketplace
- `description`: The description of the wearable that users will see in the marketplace. Make sure you indicate what the smart wearable can do, as users of the marketplace will have no way to preview its functionality before buying it. It can be at most **64 characters** long. You can write a longer description in the Builder's item editor after uploading.
- `rarity`: The rarity supply of the token. Possible values are:
  - unique (1 copy)
  - mythic (10 copies)
  - exotic (50 copies)
  - legendary (100 copies)
  - epic (1000 copies)
  - rare (5000 copies)
  - uncommon (10.000 copies)
  - common (100.000 copies)


> [!WARNING]
> **📔 Note**: If you forked your project from an existing one and it includes an `id`, make sure the value is unique before publishing your wearable. Use [uuidgenerator.net](https://www.uuidgenerator.net/) to generate a new random UUID


- `data` (required): Includes the following. `replaces`, `hides`, `tags`, `representations` and `category` must all be present. `replaces`, `hides` and `tags` can be empty lists `[]`, but `representations` needs at least one entry. You can also adjust these settings from the Builder UI once you upload the smart wearable.
  - `replaces`: List of categories of other wearables that should be unequipped when equipping this wearable, in addition to the default of this category. Eg: When putting on a cape top-body, also hide feet.
  - `hides`: List of categories of other wearables that should be hidden (but not unequipped) when equipping this wearable, in addition to the default of this category.
  - `tags`: Tags used to make the wearable searchable in the marketplace.
  - `representations`:
    - `bodyShapes`: The list of avatar representations that can use this wearable. All smart wearables must be available for both male and female body shapes.
    - `mainFile`: The main file with the 3D model of the wearable.
    - `contents`: The full list of files used to render the 3D model of the wearable. For example, the 3D model could include textures as separate files.
    - `overrideHides`: Any exceptions from the default _hide_ behavior of this wearable category.
    - `overrideReplaces`: Any exceptions from the default _replace_ behavior of this wearable category.
  - `category`: What wearable category to use. Possible values are:
    - 'eyebrows'
    - 'eyes'
    - 'facial_hair'
    - 'hair'
    - 'mouth'
    - 'upper_body'
    - 'lower_body'
    - 'feet'
    - 'earring'
    - 'eyewear'
    - 'hat'
    - 'helmet'
    - 'mask'
    - 'tiara'
    - 'top_head'
    - 'skin'
    - 'hands_wear'


> [!WARNING]
> **📔 Note**: Besides the fields above, the Builder also accepts an optional top-level `mapping` field, and optional `removesDefaultHiding` and `blockVrmExport` fields inside `data`. Smart wearables don't normally need them. The Builder rejects a `wearable.json` that has any other top-level field, with the error _"the wearable config file is invalid"_. Older versions of this page listed `menuBarIcon`, `model`, `bodyShape` and `collectionAddress`: remove them if your project still has them. The same error appears if `description` is longer than 64 characters.


## The thumbnail

You must include an image named `thumbnail.png` at root level in your folder. This image will be shown both in the backpack and the marketplace, to represent your wearable in 2d. The recommended required image size is 256x256.

Chose an image that sets player expectations and properly represents your creation.

## The Preview

Running a preview of a smart wearable is just like running that of a scene: click **Play** on the Creator Hub, or run `npm run start` on the command line. The Creator Hub can't create a smart wearable project, but it can open and preview one.

The preview runs your wearable's code as a regular scene on the parcels listed in `scene.json`, not as a portable experience. This means some behavior differs from the published wearable:

- Code that only works in the scene the player is standing in, such as [physics forces on the player](/docs/creator/sdk7/interactivity/player-physics/), stops working when you walk out of the preview's parcels. Once published, the smart wearable keeps working wherever the player goes.
- The wearable's 3D model may not show on your avatar.

To try the wearable equipped and working anywhere, upload it to the Builder and use **See in Decentraland** (see [Publish](#publish)). You don't need to mint or publish the collection to do this.

To test how the smart wearable behaves in the context of a scene, you can also run a preview of your wearable at the same time as you run a preview of one or several scenes by using a [Workspace](/docs/creator/sdk7/projects/workspaces/). For example, you can run your smart wearable together with the [Genesis Plaza](https://github.com/decentraland-scenes/Genesis-Plaza) scene to test how it behaves on a busy scene, while on an elevator, etc.

## Tips

- When positioning an entity, note that positions are global, relative to the 0,0 coordinates of Genesis Plaza.
- To react to nearby players:
  - See [Fetch all players](/docs/creator/sdk7/interactivity/user-data/#fetch-all-players) to know how to obtain data from other players in the surroundings.
  - Be mindful that the loading of the smart wearable, surrounding scenes and other players may occur in different orders depending on the situation. If the player enters Decentraland with the smart wearable already on, it’s likely that your wearable's global scene will load before other players do. On the other hand, if the player first loads into a scene and then puts on the wearable, it’s likely that other players will already be loaded by the time the wearable's scene starts running.
  - Other players only load once the player is connected to an island inside their realm. Fetch the realm data and check for the ‘room’ field. If the ‘room’ field is null, the player is not yet connected to an island and other players won’t be loaded yet. You can periodically check this every 1 second till the ‘room’ field is present, and only initialize your logic then. This only lets you see other players: to share data between wearers, see [Limitations](#limitations).
- To interact with surrounding scenes:
  - You can’t directly send any instructions to nearby scenes or other portable experiences, the `messageBus` is currently sandboxed for each portable experience/scene.
  - You can use an intermediate server to send information between the portable experience and a scene.
  - If you do a raycast, you can detect hits against the colliders of entities from the surrounding scenes. This can tell you the exact hit location, normal direction, and even the entity name and mesh name of the 3D model. This only works when hitting entities on scenes written with SDK7.
- Kill a portable experience: Run the `kill()` method to self-terminate a portable experience.
- Push the player around: [Physics](/docs/creator/sdk7/interactivity/player-physics/) functions like `Physics.applyImpulseToPlayer()` work anywhere the wearer goes, since a smart wearable's scene always counts as the scene the player is in. No permission is needed. Scenes can turn smart wearables off for their visitors, see [Feature Toggles](/docs/creator/sdk7/projects/scene-metadata/#feature-toggles).
- Play a sound only the wearer hears: attach an entity to the player with [AvatarAttach](/docs/creator/sdk7/3d-essentials/entity-positioning/#attach-an-entity-to-an-avatar) and give it an `AudioSource`. Other players don't hear it.
- Load files before they're needed: use the [AssetLoad](/docs/creator/sdk7/optimizing/pre-load-resources/) component to fetch sounds, models or emote files when the wearable starts, so the first use doesn't wait on a download.

## Publish

To publish your smart wearable:

1. Make sure the information in `wearable.json` is accurate. If you used another project as a starting point, make sure the `id` is a unique identifier, not used by other wearables.
2. Run `npm run pack` on your project folder. This generates a `smart-wearable.zip` file in your project folder.


> [!WARNING]
> **📔 Note**: The uncompressed contents of `smart-wearable.zip` must add up to less than 3MB, or the Builder rejects the file as too large. `npm run pack` prints a size message when the project goes over 2MB, but it still creates the zip. Check the size of the files in the zip, not the size of the zip itself. See [Limitations](#limitations).


3. Open the Builder, open the Collections tab, click + to upload a new wearable.
4. Drag your compressed `smart-wearable.zip` file into the Builder, verify that all the information is accurate.

> Note: If your wearable has a different 3D model for each body shape, list one representation per body shape:
>
> 1. In your project, create a new folder for each representation(`male` and `female`), and put the 3D model for each representation in its corresponding folder.
> 2. Update your `wearable.json` file to include the new representations.
> 3. ```lang-json
>    "representations": [{
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
>      "mainFile": "male/glasses.glb",
>      "contents": ["male/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    },
>    {
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
>      "mainFile": "female/glasses.glb",
>      "contents": ["female/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    }],
>    ```
> 4. Run `npm run pack` to generate a new smart-wearable.zip file.
> 5. Drag the new smart-wearable.zip file into the Builder.

5. Open wearables editor and make sure the “hide” and “remove” categories are correctly set to disable other wearable categories when this wearable is on.
6. Create a new collection with this and perhaps other wearables.
7. On the collection, click **See in Decentraland**. This opens Decentraland on the Sepolia test network, where you can equip the wearables of your collection from the backpack and see how they behave in a more real scenario, for example running around Genesis Plaza. See [Testing in Decentraland](/docs/creator/wearables-and-emotes/manage-collections/uploading-smart-wearables/#testing-in-decentraland).
8. At this point, your wearable is ready to be published.

## Permissions

Smart wearables run with restricted permissions. To use any of the following features, declare the matching permission in the `requiredPermissions` list of your `scene.json`, or the call fails at runtime:

- `USE_FETCH`: HTTP requests with `fetch` or `signedFetch`
- `USE_WEBSOCKET`: Websocket connections
- `USE_WEB3_API`: Wallet transactions or signing messages
- `OPEN_EXTERNAL_LINK`: Prompting the player to open an external link

```json
"requiredPermissions": [
    "USE_FETCH"
  ],
```

When a smart wearable declares any of these, players see an authorization prompt when they equip it, and the wearable doesn't run if they decline. Only declare what you use. See [Required Permissions](/docs/creator/sdk7/projects/scene-metadata/#required-permissions) for details.

## Limitations

> IMPORTANT: The entire smart wearable needs to fit within 3MB. This includes the 3D model, thumbnail, code, libraries, sound files, additional 3D models, UI images, etc. This limit is for the uncompressed folder. The builder will not let you upload larger wearables than this. To check the size, unzip the generated `smart-wearable.zip` file and check the size of the folder, or run `unzip -l smart-wearable.zip` and read the total.

To keep files out of the zip, list them in the project's `.dclignore` file, the same way you would for a scene.


> [!WARNING]
> **📔 Note**: If you edit your smart wearable project in the Creator Hub, it can save a full-size screenshot of the 3D view as `assets/Images/autogenerated-thumbnail.png` and point `scene.json` to it. That image can be several MB on its own and push the wearable over the limit. Smart wearables don't use it, so add `assets/Images/autogenerated-thumbnail.png` to `.dclignore`.


Smart wearables only run the portable experience for the player wearing the wearable. Other players don't see the effects. For example, if the portable experience renders a pet that follows the player, other players around won't see this pet. However, other players will see avatars perform animations that run as part of the wearable's scene, even [custom avatar animations](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#custom-animations) uploaded as part of the smart wearable's files.

Smart wearables have no built-in multiplayer. Each player wearing the wearable runs their own separate copy of its code, even when two wearers stand side by side. The `MessageBus`, syncing entities with `syncEntity()` and the [Multiplayer Server](/docs/creator/sdk7/networking/authoritative-servers/) don't work in a smart wearable. The only way to share data between wearers, like a shared score, is a server of your own that the wearable talks to with `fetch` or a WebSocket, see [Network connections](/docs/creator/sdk7/networking/network-connections/).

A custom animation can include props, and props can include particle effects made from animated bones (see [Props and Sounds](/docs/creator/wearables-and-emotes/emotes/props-and-sounds/) and [Particles in Emotes](/docs/creator/wearables-and-emotes/emotes/particles-in-emotes/)). Other players see these too. Sound is different: custom animations played with `triggerSceneEmote()` don't play audio, even if the emote file came with one. To play a sound, use an `AudioSource`, which only the wearer hears.

Smart wearables only work when players have them on. For this reason, players can only have a limited number of activated smart wearables, and depending on what part of the body they take up, some will be incompatible with others. For example, you can’t have two hats at the same time, or a helmet and hair at the same time.

If a wearable is “hidden” but not “removed” by other wearables being worn, then the smart wearable can still be on, even if the corresponding wearable is not visible.

## Examples

[Smart wearable sample](https://github.com/decentraland/smart-wearable-sample))
