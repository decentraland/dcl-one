> Create your own custom items to reuse on any of your scenes.

# Custom Items

Define a Custom Item to reuse it easily on any of your scenes, or share it with other scene creators. Custom items can consist of a single entity, or as many entities as you want. Custom items can be variations of existing Smart Items, or entirely custom, with their own tailored models and functionality.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/7cGLu8P7dso" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=7cGLu8P7dso">Watch on YouTube</a></p>



## How to Define a Custom Item

Right-click on an entity on the [Entity Tree](/docs/creator/scene-editor/get-started/scene-editor-essentials/#the-entity-tree), or select several entities and then right-click on them. Select **Create Custom Item**.

![](/docs/creator/images/editor/create-custom-item.webp)

On the lower section of the screen you are then asked to give your new Custom Item a name.

![](/docs/creator/images/editor/name-custom-item.webp)

The item is now listed on the **Custom Items** tab, at the bottom of your screen. This tab is only displayed if at least one Custom Item exists in your workspace.

![](/docs/creator/images/editor/custom-items.webp)

When defining a custom item, you can select several entities at a same hierarchical level, but not entities from separate parents if they don't share a common ancestor. Any nested entities are automatically included as part of the custom item, they don't need to be selected.

The original entities in your scene aren't affected by the action of defining a Custom Item.


> [!WARNING]
> **📔 Note**: When defining a Custom Item, you take a snapshot of the state of every component on the selected entities (except for the root entity's position, rotation, and scale). This includes **Actions**, **Triggers**, **Multiplayer**, **Scripts**, **Visibility**, and any other component.
> 
> Any smart item actions and triggers will self-reference their own copy. For example, if you define a Custom Item that includes an elevator and buttons, the buttons on each copy of the elevator will control the copy of the elevator that they belong to, not the original copy of the elevator.


### Using Custom Items

Simply drag the item from the **Custom Items** tab into your scene.

Once added, you're free to alter any property of a Custom Item, the changes you make only affect _that copy_ of the Custom Item. You can also edit or delete the original items that the Custom Item was defined from, this won't affect existing or future copies.

Notice that Custom Items are displayed with a different icon on the Entity Tree. At the top of the Item properties menu on the right, you'll also see a mention of which Custom Item they were created from.

To delete a custom item definition, right-click on the item on the **Custom Items** menu and select **Delete**. This action doesn't affect any existing copies of the item on your scenes, orphaned Custom Items remain on your scene unchanged. Deleting a Custom Item definition only removes it from the Custom Items tab.

To rename a Custom Item definition, right click on the Custom Item definition on the **Custom Items** tab and select **Rename**.

You can't modify the definition of a Custom Item that's already created, you must create a new definition and delete the original.

### Spawning Custom Items dynamically

Each Custom Item is stored as a composite, a file that describes a tree of entities and components. This means you can spawn a new copy of a Custom Item while the scene is running, instead of placing every copy by hand.

You can spawn a Custom Item in two ways:

* **With no code**: Use the **Spawn Entity** action in the Scene Editor. See [About spawning entities](/docs/creator/scene-editor/interactivity/smart-items-advanced/#about-spawning-entities).
* **With code**: Use `Composite.instance()` in the SDK. See [Composites](/docs/creator/sdk7/architecture/composites/).

### Sharing Custom Items

You can share your custom items with other creators, so they can use them on their own scenes.

Custom Items are stored each on a separate folder on your local machine. Open that folder by clicking the folder icon <img src="/docs/creator/images/editor/folder-icon.webp" alt="Code" data-size="line"> in the top right of the Custom Items tab.

You can also manually find this folder on your machine:

* In Windows: _User/AppData/Roaming/creator-hub/Custom Items_
* In Mac: _Users/username/Library/Application Support/creator-hub/Custom Items_



> [!WARNING]
> **📔 Note**: The _Library_ folder is hidden in Mac by default. The easiest way to access it is by opening Go > Go to Folder, and Typing _application support/creator-hub_


To share with someone else, simply open the Custom Items folder and copy the full folder for the item. You may want to zip the folder to make it easier to transfer. This folder contains everything needed to use your Custom Item.

The person using your Custom Item must then unzip the item folder in their own Custom Items folder on their machine. They may need to click the Refresh button <img src="/docs/creator/images/editor/refresh-icon.webp" alt="Refresh" data-size="line"> for the item to appear in their **Custom Items** tab.

Any **Assets** used by your Custom Item are also stored in the Custom Item's folder. This includes any 3D models, images, scripts, sounds, and videos referenced by the item.
