> Frequently asked questions about the Scene Editor

# Editor FAQs

Below you can find the **frequently asked questions** about the Scene Editor.

<details>
<summary><strong>How do scene limits work? What are triangles? How about materials? What's going on?</strong></summary>

Genesis City is a really, really big place. In order to make sure everyone has a smooth experience, there's a limit to how much stuff each scene can hold.

In the bottom left corner of the Scene Editor, if you click on the set of squares, you'll find a little list explaining what each of these limits are, and how far along you are to reaching each one. Let's take a look at each of these:

- **Bodies:** a body is a single mesh. For example, a bike might have three bodies: the frame and two wheels.
- **Triangles:** each surface of a body is shaped like a triangle. More complex models have more triangles than simpler models.
- **Materials:** materials make your scenes more realistic by describing how a model or shape should look. They change the way light is reflected (or emitted) from different models, and can include one or more textures.
- **Textures:** these are the images used in materials. Textures are images of different patterns and colors - like wood, stone, or grass.
- **Entities:** an entity can include one or more bodies, like the bike in the example above. Entities include everything you need for an asset: the bodies, materials, and textures.

</details>

<details>
<summary><strong>Can I upload custom assets?</strong></summary>

Yes, you can import 3D models in `.gltf` and `.glb` formats. See [**Import items**](/docs/creator/scene-editor/build/import-items/).

</details>

<details>
<summary><strong>Can I deploy my scene to my land?</strong></summary>

Yes, you can deploy to land you own, or land where you have deploy permissions. You can also deploy to a Decentraland WORLD if you own a Decentraland NAME or an ENS name. To do this, open your scene project and click the **Publish** button.

See [**Publish scene**](/docs/creator/scene-editor/publish/publish-scene/).

</details>

<details>
<summary><strong>Can I move items underground?</strong></summary>

Yes, you can place an object underground, or partially underground. However, the floor on the scene can't be removed, so you can't do tunnels or holes for players to see what's underground.

</details>

<details>
<summary><strong>How do I add images to my scene?</strong></summary>

You can import an image as you would import a 3D model. Then you can assign this image as a texture on a plane. See [**Import items**](/docs/creator/scene-editor/build/import-items/).

You can also reference an external image by URL in one of your materials. Make sure that the image is hosted in a site that has open policies about their content shared on other sites, otherwise it may not be displayed.

</details>

<details>
<summary><strong>Can I share my scenes with other users of the Scene Editor?</strong></summary>

Scenes are stored on your local disk, which allows you to upload the files to any sharing platform, like Google Drive or GitHub.

</details>

<details>
<summary><strong>How do I save projects?</strong></summary>

Projects are saved automatically to your local disk. Any change you make on the Scene Editor UI is immediately saved.

</details>

<details>
<summary><strong>Can I import scenes from the SDK?</strong></summary>

Yes, any Decentraland scene built with SDK7 can be imported into the Scene Editor. This includes scenes built with the Web Editor or entirely in code with the SDK. See [**Manage scenes**](/docs/creator/scene-editor/get-started/manage-scenes/#import-a-scene).

</details>

<details>
<summary><strong>Can I group objects?</strong></summary>

You can multi-select objects by pressing _Control_ (or _⌘ Cmd_ on Mac) and keeping it pressed while selecting more, and then apply actions to that group.

You can also nest items, so that any change to the parent's transform also affects the children. For example, you could set books as children of a bookshelf, so that moving the bookshelf also brings books with it.

</details>

<details>
<summary><strong>Can I snap/attach items to other items?</strong></summary>

Item-to-item snapping isn't available, but the Scene Editor has a **Snap to grid** feature, enabled by default, that makes items move, rotate and scale in fixed increments. You can configure the increments or toggle snapping off in the gizmos settings panel, and holding **Shift** temporarily inverts the snapping behavior for more precise placement. See [Scene editor essentials](/docs/creator/scene-editor/get-started/scene-editor-essentials/#position-items).

</details>

<details>
<summary><strong>How does Preview mode work?</strong></summary>

Use the W, A, S, and D keys to move around in Preview mode, and Space to jump.

If you can't move, you may be stuck inside an item's geometry. Check your scene's spawn points, you might have to move them to avoid starting stuck.

</details>

<details>
<summary><strong>Can I pick the color or texture of items?</strong></summary>

Yes. For items with a 3D model, add a **Swap Material** component to replace the texture or color of the model, either on the whole model or on specific parts of it. For primitive shapes, edit the **Material** component. You can also always export a 3D model, edit it in a 3D modeling tool, and import it again as a custom item.

</details>

<details>
<summary><strong>Where can I find the default 3D models in the Scene Editor's asset packs, in case I want to edit them?</strong></summary>

The model's files get added to your project's folder on your local machine as soon as you drag an item into your scene. You'll find all your scene's models under the sub folder `./assets/asset-packs/<item name>` inside the project folder.

You can also find all of these models in the [creator-hub repo](https://github.com/decentraland/creator-hub/tree/main/creator-hub/packages/asset-packs/packs).

Before editing the models, see the [3D Modeling section of our docs](/docs/creator/3d-modeling/3d-models/).

</details>

<details>
<summary><strong>What's the difference between Worlds and LAND?</strong></summary>

Worlds are personal 3D spaces located beyond the boundaries of Genesis City, while LAND refers to parcels within Genesis City's map. Here are the key differences:

* **Access**: Worlds are accessed via link/URL, while LAND scenes are found at specific coordinates on the Genesis City map
* **Size limits**: Worlds have dynamic storage capacity (based on your holdings for Decentraland NAMEs, or 36MB fixed for ENS domains), while LAND scenes have 15MB per parcel
* **Parcel limitations**: Worlds can use any layout of up to 300x300 parcels without owning them, while LAND scenes are constrained to the adjacent parcels you own or have permissions on
* **Cost**: A Decentraland NAME costs 100 MANA, which is typically much less than buying LAND parcels

See [Publishing options](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds) for more information.

</details>

<details>
<summary><strong>How do I access a World?</strong></summary>

Once a scene is uploaded to the Worlds server, you can access it in several ways:

* **Friendly URL**: Use `https://decentraland.org/jump/?realm=NAME.dcl.eth`, replacing `NAME` with the Decentraland NAME or ENS Domain
* **Chat command**: Type `/goto NAME.dcl.eth` in the chatbox
* **Explorer URL**: Use `decentraland://?realm=my-name.dcl.eth` (replace with your actual NAME)

See [Publishing to Worlds](/docs/creator/sdk7/publishing/publishing/#accessing-a-world) for more details.

</details>

<details>
<summary><strong>Can I migrate a World to Genesis City?</strong></summary>

Yes, if you are a LAND owner, you can deploy a World scene to Genesis City. You just need to re-deploy your scene to the decentralized Catalyst network (the targeted content server for Genesis City).

Things to remember:

* Remove the `worldConfiguration` section from `scene.json`
* The size limitation for Worlds (dynamic based on holdings) is different from that for LAND parcels (15MB per parcel), so make sure your scene is sized correctly for deployments to Genesis City!

See [Publishing to Worlds](/docs/creator/sdk7/publishing/publishing/#migrating-a-world-to-genesis-city) for more information.

</details>

<details>
<summary><strong>How do size limits work for Worlds?</strong></summary>

The maximum file size you can upload to your World depends on whether you're using a Decentraland NAME or an ENS domain:

**Decentraland NAMEs**: Dynamic storage capacity based on your wallet holdings:
* Each Decentraland NAME you own grants 100 MB of storage capacity
* Each Decentraland LAND parcel you own grants an additional 100 MB
* For every 2,000 MANA held in your wallet, an additional 100 MB is granted

**ENS Domains**: Fixed maximum scene file size of 36 MB per World, regardless of your other Decentraland holdings.

The space can be distributed across multiple Worlds as you wish. If you exceed your allocated storage space, you'll have a 48-hour grace period to address the situation before your Worlds become inaccessible.

You can check your used and remaining storage budget in the **Manage** section of the Creator Hub, or in the **Worlds** tab of the [Builder](https://decentraland.org/builder/worlds).

See [Worlds size limits](/docs/creator/sdk7/projects/kinds-of-project/#size-limits) for detailed information.

</details>

<details>
<summary><strong>I edited my World's name, description, or thumbnail in the Manage tab, but it changed back after I published. Why?</strong></summary>

A World has its own metadata (name, description, and thumbnail), separate from the metadata of each scene published to it.

If your World contains a single scene, the World's metadata is updated to match the scene's metadata every time you publish. Any changes made in the World's **Settings** in the **Manage** tab will be overwritten on the next publish. To make the changes stick, edit the scene's settings instead (the pencil icon in the Scene Editor).

If your World contains multiple scenes, the World's metadata is independent from the scenes, and the **Manage** tab is the only place to edit it.

See [World metadata vs scene metadata](/docs/creator/scene-editor/publish/publish-scene/#world-metadata-vs-scene-metadata) for details.

</details>
