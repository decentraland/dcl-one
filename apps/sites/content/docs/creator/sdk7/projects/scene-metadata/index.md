> Learn how to set up a scene and configure its metadata.

# Scene Metadata

A scene is a Decentraland project that is spatially delimited, and is mapped to one or several parcels. If a scene is deployed to the Decentraland Genesis City map, players can experience it by visiting the scene's coordinates. If a scene is deployed to a [World](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), players can visit it via URL.

See [Files in a scene](/docs/creator/sdk7/projects/scene-files/) for a list of what files are used in a scene project.

## Metadata

To edit a scene's metadata in the [Scene Editor in Creator Hub](/docs/creator/scene-editor/get-started/about-editor/), open a scene and click the **pencil icon**.

![](/docs/creator/images/editor/pencil-icon.webp)

This opens up the scene menu, where you can configure multiple properties.

![](/docs/creator/images/editor/metadata-ui.webp)

Alternatively, you can directly edit the `scene.json` file, where all of these values are stored.


> [!WARNING]
> **📔 Note**: Do not add custom fields to the `scene.json` file that are not mentioned in this page, as it could cause issues loading your scene.


## Scene title, description, and image

It's very important to give your scene a title, a description, and a thumbnail image to attract players and help them know what to expect.

Players will see these displayed on a modal when they select the parcels of your scene on the map. They will also see these in a confirmation screen when being [teleported](/docs/creator/sdk7/interactivity/external-links/) there by another scene. Setting up compelling data here can significantly help drive traffic to your scene.

When players navigate the world and enter your scene, they are able to read the scene title from under the minimap.

![](/docs/creator/images/media/scene-name.webp)

Add this data via the scene menu in the Scene Editor.

In case you want other developers to be able to reach out to you, you can also add contact information to your scene.


> [!NOTE]
> **📔 Note**: Worlds keep their own name, description, and thumbnail, separate from the scene's metadata. If a World contains a single scene, the World's metadata is updated from the scene's metadata on every publish. If it contains multiple scenes, the World's metadata is independent and can only be edited in the **Manage** tab of the Creator Hub. See [World metadata vs scene metadata](/docs/creator/scene-editor/publish/publish-scene/#world-metadata-vs-scene-metadata).


### Scene thumbnail

The thumbnail is set on the `navmapThumbnail` field. It must be a _.png_ or _.jpg_ image with a **16:9** aspect ratio. _1920x1080_ pixels is the size to aim for. Other sizes work as long as they keep the same 16:9 proportions.

Some parts of the platform display a reduced square version of the thumbnail, cropped to the central _1080x1080_ pixels. Keep anything essential, like text or logos, inside that safe area so that it's not cut off.


> [!WARNING]
> **📔 Note**: The Creator Hub refuses images that aren't 16:9. If you set `navmapThumbnail` by hand to an image with another shape, it may be stretched or cropped unpredictably by the clients that show it. See [Scene thumbnail](/docs/creator/scene-editor/configure/scene-settings/#scene-thumbnail).


![Scene thumbnail safe area: the full 1920x1080 image, with the central 1080x1080 square marked as the safe area and the left and right bands marked as cropped](/docs/creator/images/scene-thumbnail-safe-area.webp)

The image on `navmapThumbnail` should be a path to an image file in the project folder. It can also be a URL link to an image hosted elsewhere.


> [!WARNING]
> **📔 Note**: If you host an image elsewhere, make sure this is in a site that has permissive CORS policies for displaying content on other sites.


## Categories

You can add categories to your scene to help players explore Decentraland. These are used in the [Decentraland Places dApp](https://places.decentraland.org) to categorize each place and make it easier for players to find what they're interested in.

**Categories** need to be chosen from a pre-defined list of options:

* 🎨 Art
* 🕹️ Game
* 🃏 Casino
* 👥 Social
* 🎶 Music
* 👠 Fashion
* 🪙 Crypto
* 📚 Education
* 🛍️ Shop
* 🏢 Business
* 🏅 Sports
* 🏃 Parkour

A scene can belong to more than one category, with a maximum of 3 listed categories.

In the `scene.json` categories are listed in the `tags` array.

These are the predefined categories:

* `art`
* `game`
* `casino`
* `social`
* `music`
* `fashion`
* `crypto`
* `education`
* `shop`
* `business`
* `sports`
* `parkour`

For example, a scene could be tagged as `game` and `casino` by adding the following to the `scene.json`

```json
  "tags": [
    "game",
    "casino",
  ],
```

After that, the scene is listed on the Places dApp under the `game` and `casino` categories.

## Age Rating

The **Age Rating** field is used to classify the content of your scene. Decentraland is an 18+ platform. The following option is available:

* **🟡 `A` for Adults (18+)**: This is the minimum age requirement as specified in Decentraland's [Terms of Use](https://decentraland.org/terms). Choose this category if your scene features content appropriate for adults, such as moderate or intense language, violence, explicit content, gambling, or substances like alcohol, tobacco, and drugs.

When editing the Age Rating via the `scene.json`, rating is a **single-letter code**, write **A** for adults. The `rating` field goes at the root level of the json tree.

![](/docs/creator/images/media/content-moderation-flag-icon.webp)

```json
 "rating": "A"
```

### Restricted Content

There is a third category for scenes: 🔴 `R` for Restricted. This rating is manually applied by Content Moderators to scenes that violate Decentraland's [Content Policy](https://decentraland.org/content). Violations may include, but are not limited to:

* Suspicious content or spam
* Abusive or hateful content
* Sexual or degrading content
* Child abuse
* Harassment or bullying
* Promotion of terrorism/violence
* IP/Copyright infringement

Scenes with this rating won't load and no one will be able to interact with them. If your scene falls into this category, you should review and update it to comply with the [Content Policy](https://decentraland.org/content).

## Feature Toggles

There are certain features that can be disabled in specific scenes so that players can't use these abusively. Configure these on the **Settings** tab of the scene settings.

![](/docs/creator/images/editor/scene-restrictions.webp)

Currently, only the following feature is handled like this:

* **Voice Chat**: Refers to players using their microphones to have conversations over voice chat with other nearby players.
* **Disable Portable Experiences**: This setting will set the behavior for any portable experience of a player while standing inside your scene. This includes not only [portable experiences](/docs/creator/sdk7/projects/portable-experiences/) but also [smart wearables](/docs/creator/sdk7/projects/smart-wearables/). With this setting, you can chose to either keep them all enabled (default), disable them, or hide their UI. This is useful for scenes where portable experiences might give an unfair advantage to some players, for example using a jetpack in a parkour challenge. It's also recommended to prevent these in scenes where blockchain transactions take place, and where a malicious portable experience could potentially impersonate the scene´s UI.

On the `scene.json` file, these toggles are managed under `featureToggles`. The corresponding features are enabled by default, unless specified as _disabled_ in the `scene.json` file.

```json
"featureToggles": {
    "voiceChat": "disabled",
    "portableExperiences": "enabled" | "disabled" | "hideUi"
},
```

If a `featureToggles` property doesn't exist in your `scene.json` file, create it at root level in the json tree.

### Tipping

You can receive tips from players who visit your scene. To enable tipping, got to the **Details** tab on the scene settings and provide an Ethereum address under **Creator wallet address**.

![](/docs/creator/.gitbook/assets/creator-wallet-address.webp)


You can also provide your scene's tipping address on the `scene.json` file, under the `creator` field.

```json
  "creator": "0x1234567890123456789012345678901234567890"
```

When a player visits your scene, they will see a piggy bank icon on the top-left of the screen. Clicking on it opens a modal where they can send you a tip. This menu can also be accessed by opening your scene's info on the map.

![](/docs/creator/.gitbook/assets/TipIcon.webp)

The tip modal allows the player to select the amount of MANA they want to send. The player must own MANA in their wallet to send a tip. If the address you provided is linked to a Decentraland NAME, this modal will show the name of the wallet owner besides the Ethereum address.

![](/docs/creator/.gitbook/assets/tipping-modal.webp)


You will receive a notification on the Decentraland notifications tab whenever a player sends you a tip.

## Spawn location

The **Spawn Settings** in the **Settings** tab define where players spawn when they access your scene directly, either by directly typing in the coordinates into the browser or teleporting.

![](/docs/creator/images/editor/spawn-point-ui.webp)

Your scene might have objects that can block players from moving if they spawn on top of them, like trees or stairs. Your scene might also have elevated terrain. To prevent players from spawning in locations where they can't move, you can set multiple spawn positions in specific locations.

The position consists of coordinates inside the scene. These numbers refer to a position within the parcel, similar to what you'd use in the scene's code in a Transform component to [position an entity](/docs/creator/sdk7/3d-essentials/entity-positioning/).


> [!WARNING]
> **📔 Note**: All spawn points must be within the parcels that make up the scene. You can't spawn a player outside the space of these parcels.


Check the **Random Offset** box to randomly offset spawning players around the spawn point. This prevents all players from appearing overlapping each other when they spawn, which looks especially bad in crowded scenes. The **Max Offset** value is the maximum possible distance from the original spawn point, in both the X and Z axes.

Set the **Camera Target** to control the direction players face when they enter your scene. This gives you better control over their first impression and can help steer them towards a specific direction. By default this points at `{x: 8, y:1, z:8}`, which is the center of the scene for single-parcel scenes, or the center of the bottom-left parcel for larger scenes.

Click **Add Spawn Point** to list as many spawn points as you want. Players will randomly appear in one of those.

### Spawn points in JSON

Spawn points can also be configured via the `scene.json` file, on the `spawnPoints` field.

```json
  "spawnPoints": [
    {
      "name": "spawn1",
      "position": {
        "x": 5,
        "y": 1,
        "z": 4
      }
    }
  ],
```

A single scene can have multiple spawn points. This is especially useful in large scenes. To have many spawn points, simply list them as an array.

```json
  "spawnPoints": [
    {
      "name": "spawn1",
      "position": {
        "x": 5,
        "y": 1,
        "z": 4
      }
	},
	{
      "name": "spawn2",
      "position": {
        "x": 3,
        "y": 1,
        "z": 1
      }
    }
  ],
```

When there are multiple spawn points, the one closest to the coordinates indicated by the player is selected.

If a spawn point is marked as `default`, it will always be used, regardless of whether it's the closest. If multiple spawn points are marked as `default`, the closest one is selected.

```json
  "spawnPoints": [
    {
      "name": "spawn1",
      "default": true,
      "position": {
        "x": 5,
        "y": 1,
        "z": 4
      }
	},
	{
      "name": "not-used",
      "position": {
        "x": 3,
        "y": 1,
        "z": 1
      }
    }
  ],
```

**Spawn regions**

You can set a whole region in the scene to act as a spawn point. By specifying an array of two numbers for each dimension of the position, players will appear in a random location within that range. This helps prevent entering players from overlapping. Note that all three dimensions must use the same format: either all single numbers, or all arrays of two numbers. To keep a dimension fixed, use an array with the same number twice, like `"y": [1,1]`.

```json
  "spawnPoints": [
    {
      "name": "region",
      "position": {
        "x": [1,5],
        "y": [1,1],
        "z": [2,4]
      }
    }
  ],
```

In the example above, players may appear anywhere in the square whose corners are on _1,1,2_ and _5,1,4_.

A scene can also have multiple spawn regions, just like it can have multiple spawn points.

```json
  "spawnPoints": [
    {
      "name": "region1",
      "position": {
        "x": [1,5],
        "y": [1,1],
        "z": [2,4]
      }
    },
      {
      "name": "region2",
      "position": {
        "x": [1,5],
        "y": [1,1],
        "z": [6,8]
      }
    }
  ],
```

**Rotation**

You can also specify the rotation of players when they spawn, so that they're facing in a specific direction.

Simply add a `cameraTarget` field to the spawn point data. The value of `cameraTarget` should reference a location in space, with _x_, _y_ and _z_ coordinates relative to the scene, just like the `position` field.

```json
  "spawnPoints": [
    {
      "name": "spawn1",
      "position": {
        "x": 5,
        "y": 1,
        "z": 4
      },
      "cameraTarget": {
        "x": 10,
        "y": 1,
        "z": 4
      }
    }
  ],
```

This example spawns a player at _5, 1, 4_ facing East toward _10, 1, 4_. If the spawn position is a range, the player's rotation will always match the indicated target. If there are multiple spawn points, each can have its own separate target.

## Required Permissions

The `requiredPermissions` property manages various controlled features that could be used in an abusive way and damage a player's experience.


> [!WARNING]
> **📔 Note**: Permissions are only relevant in [portable experiences](/docs/creator/sdk7/projects/portable-experiences/) and [smart wearables](/docs/creator/sdk7/projects/smart-wearables/). Normal scenes (both in parcels or in Worlds) are not affected by these permissions, and are free to use the corresponding functionality.


These features are blocked from use in the scene unless the permission is requested in the `scene.json` file.

```json
"requiredPermissions": [
    "ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE",
		"OPEN_EXTERNAL_LINK",
  ],
```

The following permissions can be requested:

* `ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE`: Refers to [moving a Player](/docs/creator/sdk7/interactivity/avatars/move-player/)
* `ALLOW_TO_TRIGGER_AVATAR_EMOTE`: Refers to [Playing emotes on the player avatar](/docs/creator/sdk7/interactivity/avatars/avatar-animations/)
* `ALLOW_MEDIA_HOSTNAMES`: _(legacy)_ Refers to streaming media (like [video](/docs/creator/sdk7/media/video-playing/) or [audio](/docs/creator/sdk7/media/audio-streaming/)) from external domains.
* `USE_WEB3_API`: Refers to interacting with the player's browser wallets, to make transactions or sign messages.
* `USE_FETCH`: Refers to sending http requests to 3rd party servers, using `fetch` or `signedFetch`
* `USE_WEBSOCKET`: Refers to opening websocket connections with 3rd party servers
* `OPEN_EXTERNAL_LINK`: Refers to prompting the player to open links to external sites

If a `requiredPermissions` property doesn't exist in your `scene.json` file, create it at root level in the json tree.

When using `ALLOW_MEDIA_HOSTNAMES`, also include an `allowedMediaHostnames` field at root level, listing the domains that the scene is allowed to stream media from:

```json
"requiredPermissions": [
    "ALLOW_MEDIA_HOSTNAMES"
  ],
"allowedMediaHostnames": [
    "somehost.com",
    "otherhost.xyz"
  ],
```


> [!WARNING]
> **📔 Note**: `ALLOW_MEDIA_HOSTNAMES` and `allowedMediaHostnames` are legacy settings, you shouldn't need to add them. They were only enforced by older versions of the web client; current Decentraland clients play external media without them.


## Scene parcels

When [deploying](/docs/creator/sdk7/publishing/publishing/) a scene, the content is uploaded to the coordinates assigned in the scene configuration. A scene can include a single parcel, or a list of up to dozens of them.

Edit this on the second tab of the scene menu in the Scene Editor.

![](/docs/creator/images/editor/scene-parcels-3x3.webp)

Use the dropdowns and click **Apply Layout** to change the dimensions of your scene. You can also click each individual parcel to toggle it off from your layout.

![](/docs/creator/images/editor/scene-parcels-toggled.webp)

The default scene has its coordinates set to _0,0_. You don't need to change this while developing a scene offline, unless you need to occupy multiple parcels. You will need to change this before deploying to coordinates where you have deploy permissions.

You can also change the scene coordinates on the `scene.json` file:

```json
 "scene": {
    "parcels": [
      "54,-14"
    ],
    "base": "54,-14"
  }
```

The `base` field defines which parcel to consider the base parcel. If your scene has a single parcel, the base should be that parcel. If your scene has multiple parcels, the base should be the bottom-left (South-West) parcel. All entity positions are measured relative to the South-West corner of this parcel.

To display multiple parcels in the scene preview, list as many parcels as you intend to use. They don't need to be the exact parcels you'll deploy to, but they should all be adjacent and arranged in the same relative positions.

```json
 "scene": {
    "parcels": [
      "54,-14",  "55,-14"
    ],
    "base": "54,-14"
  }
```


> [!WARNING]
> **📔 Note**: The largest scene size you can set is of 300 x 300 parcels (90,000 parcels total).


### Set parcels via the command line

You can set the parcels in your scene by running the `npx update-parcels` command in your scene folder. This is especially useful for large scenes, as you don't need to list every parcel involved.


> [!WARNING]
> **📔 Note**: `update-parcels` is a third-party community tool, it's not part of the official Decentraland SDK and is not maintained by the Decentraland Foundation.


**Single parcel**

Pass a single argument with the scene coords. This coordinate is also set as the base parcel.

`npx update-parcels <parcel>`

For example:

`npx update-parcels 15,-26`

**Multiple parcels**

Pass two arguments: the South-West and the North-East parcels. The South-West parcel is also set as the base parcel.

`npx update-parcels <parcel> <parcel>`


> [!NOTE]
> **💡 Tip**: The South-West parcel is always the one with the lowest numbers on both the _X_ and _Y_ coordinates.


For example:

`npx update-parcels 15,-26 17,-24`

This command generates a 3x3 scene, with its base parcel in `15,-26`.

**Customize Base Parcel**

Pass three arguments: the South-West and the North-East parcels, and the parcel to use as a base parcel.

`npx update-parcels <parcel> <parcel> <parcel>`


> [!WARNING]
> **📔 Note**: The base parcel must be one of the parcels in the scene.


**Non-square scenes**

The above commands all generate rectangular-shaped scenes. Decentraland scenes can have L shapes or other configurations. You can generate a larger square with `npx update-parcels` and then manually remove excess parcels from the `scene.json` file.


> [!WARNING]
> **📔 Note**: The base parcel must be one of the parcels in the scene.


## Skybox time of day

You can set a fixed time of day for your scene. All players will see the scene with this time of day, and the skybox will not follow the day/night cycle.

Open the scene settings and click on the **Settings** tab to find the **Skybox** section. Uncheck the **Auto** option and set the time of day you want.

You can also set the skybox time of day in your scene code. To do this, add the following section to your `scene.json` at root level:

```json
 "skyboxConfig": {
    "fixedTime": 36000
  }
```

The number refers to the number of seconds since the start of the day, ranging from 0 (representing _00:00_) to 86400 (representing _24:00_). Any number higher than 86400 is also interpreted as midnight.

Here are some more examples of valid values:

* 0 seconds => _00:00_
* 21600 seconds => _06:00_
* 43200 seconds => _12:00_
* 64800 seconds => _18:00_
* 86400 seconds => _24:00_

## Landscape terrain

Scenes published to a [Decentraland World](/docs/creator/worlds/about/) are surrounded by an auto-generated landscape of grassland, trees, and sea. If this landscape doesn't match the aesthetics of your scene, for example a scene that floats in open water or in space, you can disable it. To do this, add the following field to your `scene.json` at root level:

```json
{
	"landscapeTerrain": false
}
```

With `landscapeTerrain` set to `false`, nothing is rendered beyond your scene's parcels, and players only see the skybox around the scene. Removing the surrounding terrain also frees up rendering resources, which can help your scene run smoother.

If the field is not set, it defaults to `true` and the landscape is displayed as usual.


> [!WARNING]
> **📔 Note**: This field only applies to Worlds that contain a single scene. It's ignored in Genesis City, where the surroundings are always determined by the neighboring parcels. You can also use it while running a local preview of your scene, to check how the scene looks without the landscape.


## World configuration

When publishing to a [Decentraland World](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), you can configure several World-specific settings in your `scene.json` file using the `worldConfiguration` object.

### Basic World configuration

To publish to a World, you must specify the NAME or ENS domain in your `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth"
	}
}
```

The **name** specified can be either a Decentraland NAME or an ENS Domain and must be owned by the wallet signing the deployment (or by any wallet that has been given permission via Access Control Lists).


### Communication service configuration

The `fixedAdapter` property indicates which Communication Service should be used by the scene. For the time being, only the `offline:offline` value is allowed. When set, the scene will have no Communication Service at all, and each user joining that world will always be alone. If not set, the Worlds content server will generate a proper value based on how it is configured.

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"fixedAdapter": "offline:offline"
	}
}
```


> [!WARNING]
> **📔 Note**: Setting `fixedAdapter` to `"offline:offline"` (or enabling **Single Player** in World Settings via the Creator Hub) disables live streaming. The streaming feature relies on the communications layer to deliver video to viewers. If you need live streaming in your world, do not use this option.


### Places listing configuration

All Worlds are automatically listed on the Places page unless you opt out. If you wish to opt-out from your Worlds being indexed in Places, you can add the following:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"placesConfig": {
			"optOut": true
		}
	}
}
```

### Complete example

Here's a complete example with all World configuration options:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"fixedAdapter": "offline:offline",
		"placesConfig": {
			"optOut": true
		}
	}
}
```

See [Publishing to Worlds](/docs/creator/sdk7/publishing/publishing/#publishing-to-worlds) for more information on deploying to Worlds.

## Fetch metadata from scene code

[Scene API Reference](https://js-sdk-toolchain.pages.dev/modules/js_runtime_apis.__system_Scene_)

You may need your scene's code to access fields from the scene metadata, such as the parcels the scene is deployed to, or the spawn point positions. This is especially useful for scenes that are meant to be replicated, or for code that is meant to be reused in other scenes. It's also very useful for libraries that need to know where the scene limits are.

To access this data, first import the `getSceneInformation` function:

```ts
import { getSceneInformation } from '~system/Runtime'
```

Then you can call the `getSceneInformation()` function, which returns a JSON object that includes most of the contents of the scene.json file. The example below shows how to access several of the more common fields from this function's response:

```ts
import { getSceneInformation } from '~system/Runtime'

executeTask(async () => {
  const sceneInfo = await getSceneInformation({})

  if (!sceneInfo) return
  console.log("SCENE INFO: ", sceneInfo)
})
```


> [!WARNING]
> **📔 Note**: `getSceneInformation()` needs to be run as an [async function](/docs/creator/sdk7/programming-patterns/async-functions/), since the response may delay a fraction of a second or more in returning data. Do not use the deprecated `getSceneInfo()` function.


The object returned by `getSceneInformation()` includes the following:

* `baseUrl`: The base URL where the scene's content is hosted
* `content`: An array with all the files of the scene, including their hash, that can be used together with the baseUrl to retrieve them.
* `metadataJson`: The full contents of the scene's scene.json, as a string. You must parse this to obtain specific values.
* `urn`: The unique urn for the scene as a whole.

The example below parses the contents from `metadataJson` to obtain values from properties in the scene.json file

```ts
import { getSceneInformation } from '~system/Runtime'

executeTask(async () => {
	const sceneInfo = await getSceneInformation({})

	if (!sceneInfo) return

	const sceneJson = JSON.parse(sceneInfo.metadataJson)
	const spawnPoints = sceneJson.spawnPoints
	const parcels = sceneJson.scene.parcels
	console.log({ parcels, spawnPoints })
})
```
