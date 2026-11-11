> How to publish your scene to LAND or a NAME from the Creator Hub.

# Publish Scene

## Before you begin

Make sure of the following:

- Your scene complies with all of the [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/). Most of these are validated each time you run a preview of your scene.
- You have a [Metamask](https://metamask.io/) account, with your LAND parcels or NAME assigned to it.
- You own the necessary amount of adjacent LAND parcels or a Decentraland NAME. Otherwise you can purchase LAND in the [Marketplace](https://decentraland.org/marketplace/) or a NAME in the [Marketplace](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> **📔 Note**: Multi-parcel scenes can only be deployed to adjacent parcels.


Check your [scene's details](/docs/creator/scene-editor/configure/scene-settings/#scene-details), make sure you provide an appealing name, description, thumbnail, categories, etc.


> [!CAUTION]
> **❗Warning**: When planning live events, don't make last minute changes to the scene right before the event. Publish your final version at least an hour before the event starts.
>
> Players who already had loaded the scene in the current session before your updates will continue to see a cached version of the scene. To see the renewed version they must close and re-enter Decentraland.


## Publish your scene

To publish your scene:

1. Open your scene in the Scene Editor and click **Publish**. This opens a window showing details about the publication.
2. Select if you want to publish to LAND or to a WORLD. See [Kinds of projects](/docs/creator/sdk7/projects/kinds-of-project/) to better understand the different options.

![](/docs/creator/images/editor/publish-options.webp)

3. If publishing to LAND, select the location on the map. You'll see your eligible parcels marked in red. If publishing to a WORLD, you'll see your eligible NAMEs in a dropdown. The scene is published to the parcels or the World you pick here, whatever coordinates its `scene.json` had before.


> [!NOTE]
> **💡 Tip**: If you don't see your parcels or NAMEs, make sure you're connected to the Creator Hub using the right user account. Otherwise exit the project and click the user settings icon on the top-right corner, then select **Sign Out** and sign back in again.


4. The next screen shows all of the files you're currently uploading and their sizes, confirm the operation. See [Check the scene size](#check-the-scene-size).
5. The publication process will then start. It has two stages, and once both are done a **Jump In** button appears.


> [!NOTE]
> **💡 Tip**: The success screen also shows a web link to your scene, with a button to copy it. Anyone can open that link in a browser, whether or not they have the desktop client installed, so it's the one to paste into a Discord message or an event page. It looks like `https://decentraland.org/jump?realm=<your-world-name>` for a World, or `https://decentraland.org/jump?position=<x>,<y>` for a scene in Genesis City.
>
> The **Jump In** button is for you rather than your audience: it opens the scene in your own desktop client so you can check your work.


![](/docs/creator/images/editor/deploy-steps.webp)


> [!NOTE]
> **📔 Note**: The two stages of the deployment involve:
>
> - **1. Uploading**: Uploading the files to the servers. Scenes of 50 MiB or more upload in batches, see [Publish large scenes](/docs/creator/sdk7/publishing/publishing/#publish-large-scenes).
> - **2. Converting**: The scene's 3D models are compressed into Asset Bundles for faster rendering. This usually takes seconds. It may take longer for very large scenes.
>
> Decentraland's servers also generate Low Level of Detail (LOD) versions of your models, used to draw your scene from far away. That runs in the background once your scene is already live, so publishing never waits for it.


The scene itself is live as soon as the success screen appears. Its card in the explorer (title, description and thumbnail) is indexed separately and can take a few minutes to catch up.

### Which LAND you can publish to

The map only offers parcels and Estates your wallet can publish to right now: LAND you own, LAND where you're a collaborator or a manager, and LAND you rent. A chip on the publish screen shows whether you're the **Owner** or a **Collaborator** of the LAND you picked.

LAND you rented out to someone else doesn't appear, and neither does LAND where your collaborator or manager permission was removed. To manage these permissions, see [Manage LANDs and Worlds](/docs/creator/scene-editor/publish/manage-lands-and-worlds/).

### Check the scene size

The file list shows the scene's **Total Size** next to the size it may use, for example `Total Size 12.40 MB / 15.00 MB`:

- **Genesis City**: 15 MiB per parcel the scene covers.
- **Worlds**: the limit the Worlds server gives for this World and this placement. See [World storage budget](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#world-storage-budget).

While the Creator Hub looks up a World's limit, the screen reads **Checking limit…**. If the scene is over the limit, the total turns red and the **Publish** button is disabled. Remove or shrink files until it fits. See [Scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/).

### Keep your Places listing

Each published scene has a listing in Decentraland Places, with its likes, favorites, ranking, [analytics](/docs/creator/sdk7/other/scene-analytics/) and [Multiplayer Server storage](/docs/creator/scene-editor/live-ops/server-data/). When you republish, Decentraland keeps that listing only if the new version clearly replaces the old one. Otherwise it registers a new place, which starts from zero.

Before publishing, the Creator Hub checks what's already deployed. If your publish would lose the listing, it shows **Your scene will lose its Places listing**, with what stays behind on the old listing. You can **Go back** and change the placement, or **Publish anyway**.

- **In Genesis City**: the warning appears when the new version changes both the base parcel and the set of parcels of the scene it replaces. To keep the listing, publish to the same base parcel or to the same set of parcels as before.
- **In a World**: the warning appears when the new scene overlaps two or more existing scenes, or when you publish as a single-scene World and the new scene doesn't cover every scene already in the World. This includes a World that holds only one scene. To keep the listing, place the scene so that it replaces exactly one existing scene.

### Follow and cancel the upload

While the scene uploads, the deploy step shows a progress bar: **Preparing the upload...**, **Uploading** with the number of files sent, **Batch** progress for large scenes, then **Publishing...**.

Click **Cancel** to stop the upload. A cancelled publish isn't an error: you go back to the previous step, and nothing is added to the scene's publish history.

The progress bar and **Cancel** need `@dcl/sdk` 7.31.0 or later in your project. With an older SDK the upload still works, but you can't follow or cancel it from here.


> [!WARNING]
> **📔 Note**: If the Creator Hub loses track of a publish that the servers already accepted, or if it doesn't finish within 60 minutes, it tells you the deployment may still finish. Check the scene in the [Manage](/docs/creator/scene-editor/publish/manage-lands-and-worlds/) page before publishing again.


### If publishing stops before it starts

- **Couldn't save your scene before publishing**: the Creator Hub saves every pending change before it publishes. If that save fails, it stops the publish so it doesn't upload an older version, and shows the reason. Fix the problem and publish again.
- **We couldn't load this world's details**: when publishing to a World, the Creator Hub reads the World's current settings, scenes and permissions first. If that fails, check your connection and click **Retry**.

## How long does publishing take?

- **Until you can test it**: seconds to a couple of minutes. It scales with the total size of your scene and your upload bandwidth. The **Jump In** button appears as soon as the scene is playable, once both stages are done.
- **Before a live event**: publish your final version at least an hour before the event starts, to leave room for anything unexpected.

The following factors can make a publication slower:

- **Total scene size**: affects both the upload and the conversion.
- **The number and complexity of distinct 3D models**: models are converted one by one.
- **How busy the conversion servers are**: your scene may be queued behind other scenes published at around the same time.

## Publishing to Worlds

To rename a World, see its scenes, set its spawn point, invite collaborators or make it private, open it from the **Manage** page of the Creator Hub. See [Manage LANDs and Worlds](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#manage-a-world).

Scenes in Worlds count against a storage budget shared by all the Worlds your wallet owns. See [World storage budget](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#world-storage-budget).

### World metadata vs scene metadata

A World and each scene published to it have two separate sets of metadata. Both include a name, a description, and a thumbnail:

- **Scene metadata**: Belongs to the scene project, stored in its `scene.json` file. You edit it via the [scene settings](/docs/creator/scene-editor/configure/scene-settings/) in the Scene Editor, and it's uploaded together with the scene every time you publish.
- **World metadata**: Belongs to the World itself. You edit it on the **Details** tab of the World's page in **Manage**. This is the information shown in Decentraland Places and in the in-world World information. See [World details](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#details).

How these two relate depends on how many scenes the World contains:

- **Empty World**: When you publish a scene to an empty World, the World's metadata is filled in from the scene's metadata.
- **Single-scene World**: Every time you publish the scene, the World's metadata is updated to match the scene's metadata.
- **Multi-scene World**: The World's metadata is fully independent from the metadata of each of its scenes. Publishing a scene never changes the World's metadata. The only way to change it is on the World's **Details** tab in **Manage**.

In practice, if your World contains a single scene, you don't need to think of the World's metadata as a separate thing: just edit your scene's settings and publish, and the World will always reflect them.


> [!WARNING]
> **📔 Note**: In a single-scene World, any changes you make on the World's **Details** tab will be overwritten by the scene's metadata the next time you publish the scene. To make those changes stick, edit the scene's settings instead.


### Multi-Scene Worlds

A World can have multiple scenes, published by the World Owner or by other creators. This enables a collaborative environment where each parcel can be managed by different Collaborators.

#### Making a World Multi-Scene

A World Owner can choose to make the World Multi-Scene by toggling **Multi-Scene World (Advanced)** when publishing to a single-scene World.

<img src="/docs/creator/.gitbook/assets/multi-scene-owner-first-deploy.webp" width="600" />

Once the Multi-Scene World is published, the World Owner can publish additional scenes or [add Collaborators](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#world-collaborators) to publish within the World.


> [!NOTE]
> **📔 Note**: A Multi-Scene world size adapts automatically to contain all the published scenes, growing and shrinking dynamically on each publish. The space left between different scenes in the Multi-World is filled with environment.


#### Deploying to a Multi-Scene World as a Collaborator

When going through the publishing process, a collaborator can publish only to the parcels the World Owner gave them. If the owner set specific parcels, only those are available. If access was set to **All Parcels**, the collaborator can pick any parcel in the World.

In the **Manage** page, a collaborator sees the World with a **Collaborator** tag. What its page shows depends on their permission, see [Manage a World](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#manage-a-world).


> [!WARNING]
> **📔 Note**: Collaborators with **All Parcels** publishing access can overwrite any scene from the world, even if it was published by the owner or other collaborators.


<img src="/docs/creator/.gitbook/assets/multi-scene-publish-collaborator.webp" width="600" />

### Private Worlds

A World can be open to anyone, invitation only, or password protected. The owner sets this in the **Access** section of the World's **Settings** tab. See [Control who can enter](/docs/creator/scene-editor/publish/manage-lands-and-worlds/#control-who-can-enter).

## Publish from a hardware wallet

Instead of storing your LAND tokens in a Metamask account, you may find it more secure to store them in a hardware wallet device, such as a [Ledger](https://www.ledger.com/) or a [Trezor](https://trezor.io/), that's physically plugged in to your computer.

If you're using one of these devices, you can link the hardware wallet to Metamask to enable signing messages, while keeping the tokens more secure. See [this article from Metamask](https://support.metamask.io/more-web3/wallets/how-to-connect-a-trezor-or-ledger-hardware-wallet/) for instructions to connect your account.

Once your hardware wallet can be used via Metamask, you can deploy following the same steps as if your tokens were on a Metamask account.

## Scene overwriting

When a new scene is deployed, it overwrites older content that existed on the parcels it occupies.

Publishing normally starts right away. If you're about to replace content that already exists in a World, the Creator Hub asks you to confirm first, and offers a **Don't show again** option.

If a scene that takes up multiple parcels is only partially overwritten by another, all of its parcels are either overwritten or erased.

Suppose you deployed your scene _A_ over two parcels _\[100, 100]_ and _\[100, 101]_. Then you sell parcel _\[100, 101]_ to a user who owns adjacent land and that deploys a large scene (_B_) to several parcels, including _\[100, 101]_.

Your scene _A_ can't be partially rendered in just one parcel, so _\[100, 100]_ won't display any content. You must build a new version of scene _A_ that only takes up one parcel and deploy it to only parcel _\[100, 100]_.

## Publish to granted land

If you're publishing to land owned by the Decentraland Foundation that was granted to you via a grant, click the **Publish** button normally, then select **Publish to a different server** on the bottom. Then select **Custom Server** from the dropdown and enter the following server address: `https://linker-server.decentraland.org`.


> [!WARNING]
> **📔 Note**: You must first manually set the coordinates of your scene in the advanced tab of the Layout settings. See [Scene Settings](/docs/creator/scene-editor/configure/scene-settings/#layout) for more info.


## Custom servers

You can deploy content to a custom server that doesn't belong to the official DAO-maintained network of catalyst servers. To do this, you don't need to own any LAND or NAME tokens, as you can configure the server to use any validation logic you prefer to control who can deploy where. Custom servers can chose to have content from the official servers, that you can overwrite, or start from a blank slate and publish entirely new content.

To publish to a custom server, click the **Publish** button normally, then select **Publish to a different server** on the bottom. Then select **Custom Server** from the dropdown and enter the address of the server.

See [How to run your own Catalyst Node](/docs/contributor/tutorials/how-to-run-a-catalyst/) for more info on what you can do with your own server and how to set it up.


> [!WARNING]
> **📔 Note**: Players will need to manually type in a URL to access your custom server. Certain validations from services like the [rewards server](/docs/creator/rewards/getting-started/) might fail in these contexts, as often these services require that the request comes from an official server.


Players are never directed to this server, the only way to access it is to explicitly type in the URL to connect to it.

## Verify deployment success

Once you deployed your scene, these changes can take a few minutes to be uploaded and processed. If you enter Decentraland right after deploying, you might briefly still see the previous version of your content.

The Creator Hub displays the progress of the publication as it moves through the **Uploading** and **Converting** stages, and shows a **Jump In** button as soon as the scene is playable.

### Check the conversion status

While the 3D models of a new version are being converted, players are deliberately served the last fully-working version of your scene. Conversions should take a couple of minutes tops, if they take longer it could be a sign something failed.

You can query the conversion status directly in a browser:

- `https://asset-bundle-registry.decentraland.org/entities/status/<pointer>`: replace `<pointer>` with one of your scene's coordinates (for example `20,-34`) or with the entity ID of the deployment. The response shows the conversion status for each platform under `assetBundles`, and the status of the LODs under `lods`. When `complete` is `true`, the scene is fully converted for all platforms. For a scene in a World, add the World's name as a query parameter, for example `.../entities/status/0,0?world_name=myname.dcl.eth`.
- `https://asset-bundle-registry.decentraland.org/queues/status`: lists the IDs of all the scenes currently waiting to be converted, per platform. If your entity ID appears here, your scene is queued behind other conversions and you just need to wait a little longer.


> [!WARNING]
> **📔 Note**: If you have an open session of the Decentraland explorer where you already loaded your scene, you'll need to close and re-open the app. Otherwise you'll keep seeing the cached version.

