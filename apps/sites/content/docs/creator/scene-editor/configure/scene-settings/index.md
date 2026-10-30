> Edit your scene's metadata

# Scene Settings

Click the **Pencil icon** on the top-right of the screen. This opens a series of scene-level properties to edit.

![](/docs/creator/images/editor/pencil-icon.webp)

Here you can configure multiple properties including title and thumbnail, scene size, scene categories, and feature toggles.

See [Scene Metadata](/docs/creator/sdk7/projects/scene-metadata/).

## Scene details

The **Details** tab lets you configure several fields about your scene. These fields are shown to players that might visit your scene, for example when expanding the location on the map, when being prompted to teleport, or when sharing a link to the scene on social media. Make sure you make the information here attractive and accurate to drive more traffic to your scene!

![](/docs/creator/images/thumbnail-image.webp)

The following fields are available:

- **Name**
- **Description**
- **Thumbnail**
- **Categories**
- **Creator name**
- **Creator contact email** (optional)
- **Creator wallet address** (optional)

See [scene metadata](/docs/creator/sdk7/projects/scene-metadata/) for more details on these fields.


> [!NOTE]
> **📔 Note**: If you publish your scene to a [World](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), the World has its own name, description, and thumbnail, separate from the scene's. If the World contains a single scene, the World's metadata is updated to match the scene's details on every publish, so this panel is all you need to edit. If the World contains multiple scenes, the World's metadata can only be edited in the **Manage** tab of the Creator Hub. See [World metadata vs scene metadata](/docs/creator/scene-editor/publish/publish-scene/#world-metadata-vs-scene-metadata).



> [!WARNING]
> **📔 Note**: The scene's **Age Rating** is not edited on this panel. You can set the `rating` field directly in the `scene.json` file, or, for scenes published to a World, change the **Content Rating** in the Creator Hub's World Settings after publishing. Decentraland is an 18+ platform, so the rating to set is `A` for Adults. See [Age Rating](/docs/creator/sdk7/projects/scene-metadata/#age-rating).


### Scene thumbnail

The thumbnail must be a .png or .jpg image with a **16:9** aspect ratio. 1920x1080 pixels is the size to aim for. Other sizes work as long as they keep the same 16:9 proportions.

An image that isn't 16:9 is refused: importing one shows an error, and choosing an off-ratio image that's already in your project shows **Thumbnail not supported** and keeps the thumbnail you had.

Some parts of the platform display a reduced square version of the thumbnail, cropped to the central square. That cuts about 22% off each side. Keep anything essential, like text or logos, inside that safe area so it isn't lost.

The **Details** tab shows your thumbnail full width, with those two side bands shaded so you can see exactly what a square view drops, and the image's real pixel size underneath.

![Scene thumbnail safe area: the full 1920x1080 image, with the central 1080x1080 square marked as the safe area and the left and right bands marked as cropped](/docs/creator/images/scene-thumbnail-safe-area.webp)


> [!NOTE]
> **💡 Tip**: If you don't provide a thumbnail, the scene uses the automatic capture you see on the scene's card. That capture is cropped from the center to 16:9 and never enlarged, so it can end up smaller than 1920 wide. Uploading your own image is almost always better.


### Tipping

You can receive tips from players who visit your scene. To enable tipping, got to the **Details** tab on the scene settings and provide an Ethereum address under **Creator wallet address**.

![](/docs/creator/.gitbook/assets/creator-wallet-address.webp)

When a player visits your scene, they will see a piggy bank icon on the top-left of the screen. Clicking on it opens a modal where they can send you a tip. This menu can also be accessed by opening your scene's info on the map.

![](/docs/creator/.gitbook/assets/TipIcon.webp)

The tip modal allows the player to select the amount of MANA they want to send. The player must own MANA in their wallet to send a tip. If the address you provided is linked to a Decentraland NAME, this modal will show the name of the wallet owner besides the Ethereum address.

![](/docs/creator/.gitbook/assets/tipping-modal.webp)

You will receive a notification on the Decentraland notifications tab whenever a player sends you a tip.

## Layout

You can edit the size of your scene by clicking the _pencil icon_ and then changing the number of rows and columns.

Scenes in Decentraland occupy one or several adjacent LAND parcels. Each LAND parcel measures 16x16 meters.

Set the number of parcels for the rows and columns and click **Apply layout** for it to affect how your scene looks on the Scene Editor canvas.

![](/docs/creator/images/editor/scene-layout.webp)

To build something to deploy to LAND parcels you own, make sure the shape of the scene matches the shape of where you want it deployed.


> [!NOTE]
> **💡 Tip**: You can toggle each tile on the grid off by clicking on it. This allows you to draw non-rectangular shapes for your scene layout.
> 
> <img src="/docs/creator/images/editor/non-rectangular.webp" alt="" data-size="original">


If you own a Decentraland NAME, you can also deploy your scene to a [Decentraland World](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds). In that case, you can use any layout of up to 300x300 parcels without needing to own them, but you will have a size limit in MB.

See [Kinds of project](/docs/creator/sdk7/projects/kinds-of-project/) to better understand the different options.

### Advanced view

You can also click the **Set Coordinates (Advanced)** button to manually list the coordinates of your scene.

![](/docs/creator/images/editor/advanced-coordinates.webp)

In **Custom Coordinates**, write the coordinates of each of the parcels where you wish to publish. Separate the x and y coordinate with a comma, and each set of coordinates separated by spaces. Remember that these coordinates must all be adjacent to be valid. For example:

`78,-2 79,-2 78,-3 79,-3`

In the **Origin Point** field, define which of the coordinates in the scene should be treated as the point of origin. This has to be one of the coordinates you listed in the **Custom Coordinates** field. It's recommended to set the parcel on the bottom-left of the scene.

## Scene restrictions

You can disable certain functionalities on your scene if you chose, in case they might be abused or clash with the kind of experience you want to create.

![](/docs/creator/images/editor/scene-restrictions.webp)

- **Silence Voice Chat**: Prevent players on your scene from using voice chat.
- **Disable Nearby Voice Chat**: Prevent players on your scene from using the nearby (proximity-based) voice chat.
- **Disable Smart Wearables & Portable Experiences**: Prevent players from using [Smart Wearables](/docs/creator/sdk7/projects/smart-wearables/) or [Portable Experiences](/docs/creator/sdk7/projects/portable-experiences/).

## Skybox Control

You can control the skybox time of day in the **Settings** tab. You can set a fixed time of day for your scene. All players will see the scene with this time of day, and the skybox will not follow the day/night cycle.

In the Creator Hub, open the scene settings and click on the **Settings** tab to find the **Skybox** section. Uncheck the **Auto** option to avoid using the day/night cycle and set the time of day you want.

![](/docs/creator/images/fixed-time-of-day.webp)
