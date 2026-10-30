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

* `wearable.json` includes all of the metadata for the portable experience
* There’s a placeholder 3D model (glasses.glb) and thumbnail (glasses.png) for a pair of dark glasses. You should replace these with the actual wearable you are creating
* `scene.json` is a lot shorter, it doesn’t include properties that are irrelevant to a wearable, like parcels or spawn points

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

The following fields are required in `wearable.json`:

* `id`: Unique id of the smart wearable.
* `name`: The name for the wearable that users will see in the marketplace
* `description`: The description of the wearable that users will see in the marketplace. Make sure you indicate what the smart wearable can do, as users of the marketplace will have no way to preview its functinality before buying it.
* `rarity`: The rarity supply of the token. Possible values are:
  * unique (1 copy)
  * mythic (10 copies)
  * exotic (50 copies)
  * legendary (100 copies)
  * epic (1000 copies)
  * uncommon (10.000 copies)
  * common (100.000 copies)


> [!WARNING]
> **📔 Note**: If you forked your project from an existing one and it includes an `id`, make sure the value is unique before publishing your wearable. Use [uuidgenerator.net](https://www.uuidgenerator.net/) to generate a new random UUID


The following fields can also optionally be included. These settings can also be configured from the Builder UI, once you upload the smart wearable.

* `data`: Includes the following
  * `replaces`: List of categories of other wearables that should be unequipped when equipping this wearable, in addition to the default of this category. Eg: When putting on a cape top-body, also hide feet.
  * `hides`: List of categories of other wearables that should be hidden (but not unequipped) when equipping this wearable, in addition to the default of this category.
  * `tags`: Tags used to make the wearable searchable in the marketplace.
  * `representations`:
    * `bodyShapes`: The list of avatar representations that can use this wearable. All smart wearables must be available for both male and female body shapes.
    * `mainFile`: The main file with the 3D model of the wearable.
    * `contents`: The full list of files used to render the 3D model of the wearable. For example, the 3D model could include textures as separate files.
    * `overrideHides`: Any exceptions from the default _hide_ behavior of this wearable category.
    * `overrideReplaces`: Any exceptions from the default _replace_ behavior of this wearable category.
  * `category`: What wearable category to use. Possible values are:
    * 'eyebrows'
    * 'eyes'
    * 'facial\_hair'
    * 'hair'
    * 'mouth'
    * 'upper\_body'
    * 'lower\_body'
    * 'feet'
    * 'earring'
    * 'eyewear'
    * 'hat'
    * 'helmet'
    * 'mask'
    * 'tiara'
    * 'top\_head'
    * 'skin'
    * 'hands\_wear'
* `menuBarIcon`: Image to use on the “experiences” menu, to represent this portable experience, to represent the portable experience. This image should be at root level in your folder. The recommended image size is 256x256.
* `model`: The 3D model to use for the wearable. This file should be at root level in your folder.
* `bodyShape`: The avatar body type that this wearable is compatible with. Possible values:
  * male
  * female
  * both
* `collectionAddress`: The ethereum address of the published collection of wearables. This address is assigned once publishing, it can be left blank.

## The thumbnail

You must include an image named `thumbnail.png` at root level in your folder. This image will be shown both in the backpack and the marketplace, to represent your wearable in 2d. The recommended required image size is 256x256.

Chose an image that sets player expectations and properly represents your creation.

## The Preview

Running a preview of a portable experience is just like running that of a scene, simply click **Preview** on the Creator Hub, or run `npm run start` on the command line. If the `wearable.json` file is properly configured and the project is recognized as a smart wearable, you’ll notice that all the visible around you are the default empty parcels. In this preview mode, you are not restricted to any set of parcels, you can add 3D models or sounds anywhere in the world.

To test how the smart wearable behaves in the context of a scene, you can also run a preview of your wearable at the same time as you run a preview of one or several scenes by using a [Workspace](/docs/creator/sdk7/projects/workspaces/). For example, you can run your smart wearable together with the [Genesis Plaza](https://github.com/decentraland-scenes/Genesis-Plaza) scene to test how it behaves on a busy scene, while on an elevator, etc.

## Tips

* When positioning an entity, note that positions are global, relative to the 0,0 coordinates of Genesis Plaza.
* To react to nearby players:
  * See [Fetch all players](/docs/creator/sdk7/interactivity/user-data/#fetch-all-players) to know how to obtain data from other players in the surroundings.
  * Be mindful that the loading of the smart wearable, surrounding scenes and other players may occur in different orders depending on the situation. If the player enters Decentraland with the smart wearable already on, it’s likely that your wearable's global scene will load before other players do. On the other hand, if the player first loads into a scene and then puts on the wearable, it’s likely that other players will already be loaded by the time the wearable's scene starts running.
  * For multiplayer experiences, wait till the player is connected to an island inside their realm. Fetch the realm data and check for the ‘room’ field. If the ‘room’ field is null, the player is not yet connected to an island and other players won’t be loaded yet. You can periodically check this every 1 second till the ‘room’ field is present, and only initialize your logic then.
* To interact with surrounding scenes:
  * You can’t directly send any instructions to nearby scenes or other portable experiences, the `messageBus` is currently sandboxed for each portable experience/scene.
  * You can use an intermediate server to send information between the portable experience and a scene.
  * If you do a raycast, you can detect hits against the colliders of entities from the surrounding scenes. This can tell you the exact hit location, normal direction, and even the entity name and mesh name of the 3D model. This only works when hitting entities on scenes written with SDK7.
* Kill a portable experience: Run the `kill()` method to self-terminate a portable experience.

## Publish

To publish your smart wearable:

1. Make sure the information in `wearable.json` is accurate. If you used another project as a starting point, make sure the `id` is a unique identifier, not used by other wearables.
2. Run `npm run pack` on your project folder. This generates a `smart-wearable.zip` file in your project folder.


> [!WARNING]
> **📔 Note**: The output of `npm run pack` will indicate the size of the uncompressed exported project, it must be under 3MB. If larger than that, it won’t be accepted by the builder.


3. Open the Builder, open the Collections tab, click + to upload a new wearable.
4. Drag your compressed `smart-wearable.zip` file into the Builder, verify that all the information is accurate.

> Note: If your wearable contains different model representations, you need to do a workaround:
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
7. Hit the 3 dots icon next to “Mint Items” and select “See in world”. This will open a tab with the explorer on Sepolia, where you can try out all the wearables of your collection in Decentraland, and see how they behave in a more real scenario, for example running around Genesis Plaza.
8. At this point, your wearable is ready to be published.

## Restricted actions

To prevent abuse, certain features aren't permitted on portable experiences by default, and require adding a permission flag.

See [Required permissions](/docs/creator/sdk7/projects/scene-metadata/#required-permissions) for more details.

## Limitations

> IMPORTANT: The entire smart wearable needs to fit within 3MB. This includes the 3D model, thumbnail, code, libraries, sound files, additional 3D models, UI images, etc. This limit is for the uncompressed folder. The builder will not let you upload larger wearables than this. To check the size of your portable experience, run `npm run pack`, the project size is specified in the output text of the command. You can also verify this by uncompressing the generated `smart-wearable.zip` file and checking the size of the folder.

Smart wearables only run the portable experience for the player wearing the wearable. Other players don't see the effects. For example, if the portable experience renders a pet that follows the player, other players around won't see this pet. However, other players will see avatars perform animations that run as part of the wearable's scene, even [custom avatar animations](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#custom-animations) uploaded as part of the smart wearable's files.

Smart wearables only work when players have them on. For this reason, players can only have a limited number of activated smart wearables, and depending on what part of the body they take up, some will be incompatible with others. For example, you can’t have two hats at the same time, or a helmet and hair at the same time.

If a wearable is “hidden” but not “removed” by other wearables being worn, then the smart wearable can still be on, even if the corresponding wearable is not visible.

## Examples

[Smart wearable sample](https://github.com/decentraland/smart-wearable-sample))
