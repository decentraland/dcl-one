> How to use the Scene Editor

# Scene Editor Essentials

The Scene Editor's UI is divided into a few different sections, with different purposes.

![](/docs/creator/images/editor/editor-layout.webp)

- **Canvas**: Manipulate items directly and see what your scene looks like.
- **Entity tree**: Contains a list of all items in the scene and their hierarchy.
- **Properties**: Displays details about the currently selected item.
- **Resources**: Shows resources that are available to use.

## Moving around

To find your way around the Scene Editor:

- Use **W** and **S** to move close or far. You can also use the mouse scroll wheel, or **+** and **-** keys
- Use **A** and **D** to move sideways.
- Use **Q** and **E** to move up and down.
- Click the **Right Mouse Button** and drag to rotate the camera.
- Press **Space bar** to reset the camera back to the default position
- Use **Left Mouse Button** to click and select items and to move them around.

## Preview on mobile

The dropdown next to the **Preview** button has a **Show QR Code for Mobile** option. Scan the QR code with a phone on the same Wi-Fi network to open your scene in the [Decentraland mobile app](/docs/creator/build-for-mobile/mobile-client/overview/). This is the recommended way to validate UI, controls, and performance for mobile players. See [Preview on mobile](/docs/creator/build-for-mobile/develop/preview-on-mobile/) for the full guide.

## Set the Ground

The scene's ground can use various different textures. You can find these in the different themed asset packs in the item menu.

Items of type **Ground** have a paint bucket icon on them. If you drag one of these into your scene, it covers all of your scene's ground with copies of this item.

![](/docs/creator/images/editor/ground.webp)

You can also add a single copy of the item by holding **Shift** while you drag the ground onto the scene.

![](/docs/creator/images/editor/ground-entities.webp)

The collection of ground items appear in the [entity tree](/docs/creator/scene-editor/get-started/scene-editor-essentials/#the-entity-tree) inside a folder. Each one of them is locked, to prevent accidentally selecting. [Untoggle](/docs/creator/scene-editor/get-started/scene-editor-essentials/#lock-or-hide-items) the items to move or edit them.

## Add items

Navigate the themed asset pack categories on the menu on the bottom to find different items that you can place on your scene.

![](/docs/creator/images/editor/asset-packs.webp)

You can also use the search box. Note that when you're inside an asset pack, the search only looks in that asset pack.

To place an item, click and drag it in from the asset pack menu into a location on your scene in the canvas.

![](/docs/creator/images/editor/drop-item.webp)


> [!NOTE]
> **💡 Tip**: Your changes are saved automatically whenever you add, move, or edit properties of any of the items in your scene.


To duplicate an item, select it and hit **Ctrl + C** and then **Ctrl + V**. You can also find the item on the [entity tree](/docs/creator/scene-editor/get-started/scene-editor-essentials/#the-entity-tree) to right-click and select the option **Duplicate**. The new item will be perfectly overlapping the original.

To delete an item from the scene, select it press the _Delete_ key.

See [Import items](/docs/creator/scene-editor/build/import-items/) for adding your own custom 3D models from disk.


> [!WARNING]
> **📔 Note**: Once you dragged a 3D model into your scene, it's downloaded into your project folder and remains there even if you delete it. These unused models can increase the size of your scene.
> 
> Open the **Local Assets** tab to delete any unused models.


## Position items



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/cNl02PFPdcQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=cNl02PFPdcQ">Watch on YouTube</a></p>



Click with the **Left Mouse Button** and drag a selected item to move it freely around the scene at ground level.

You can also use the tools on the top menu:

![](/docs/creator/images/editor/gizmos.webp)

- **Move tool**: Click and drag each arrow to move the item in a single axis at a time. With this tool you can also position things above the ground level.
- **Rotate tool**: Click and drag each of the hoops around the item to rotate the item on one axis at a time.
- **Scale tool**: Click on the center of the gizmo and drag in or out to enlarge. This tool also lets you stretch an item in a single axis to change its proportions, to do this click on one of the axis of the gizmo and drag.

![](/docs/creator/images/editor/move-items.webp)

To have greater precision while moving, rotating or scaling an item, press and hold the _Shift_ key while making adjustments. This will avoid snapping to the grid.

To change the movement granularity and other settings, click the downward arrow on the right of the tools. The following settings are available:

- **Snap**: Toggle the grid on or off. When off, the behavior of **Shift** is inverted: you don't follow the grid by default, you do if you hold **Shift**.
  - **Position**: The size of movement increments in meters when **Snap** is on.
  - **Rotation**: The size of rotation increments in degree when **Snap** is on.
  - **Scale**: The size of scale increments when **Snap** is on.
- **Align to world**: A single checkbox that refers to the axis used by the gizmos. When checked, the Move and Rotate tool axis always align with the world, and don't change with the object's orientation. When unchecked, they align with the object's orientation.

To select multiple items at the same time, press and hold the _Control_ key while selecting them. You can then move, rotate, scale, duplicate or delete all of them in a single action.

### Type an exact amount

When dragging isn't precise enough, type the amount instead:

1. Press a tool key: **M** or **G** to move, **R** to rotate, **X** to scale.
2. Press **X**, **Y** or **Z** to pick an axis. Scale doesn't need one: without an axis it scales every axis by the same factor.
3. Type the number. Use **.** for decimals, and **-** to switch between positive and negative.
4. Press **Enter** to apply, or **Esc** to cancel. **Backspace** erases what you typed.

A chip at the bottom of the canvas shows the entry as you type it, tells you what is still missing, and confirms what was applied.

Some examples:

- **R** **X** **1** **5** **Enter** rotates the selection 15 degrees around X.
- **X** **2** **Enter** makes the selection twice as big.
- **M** **Y** **-** **3** **Enter** moves it 3 meters down.

A few things worth knowing:

- The amount is always relative to where the selection is now, not an absolute coordinate.
- Typed values ignore the snap grid.
- With several items selected, they all move, rotate or scale around their shared center, as a single undo step.
- Pressing **M**, **G** or **R** a second time still turns its gizmo off. **X** doesn't, because while you're typing it means the X axis.


> [!NOTE]
> **💡 Tip**: The keyboard icon in the bottom-right corner of the canvas opens the full list of shortcuts.


## Smart items

Smart items are special items that come with built-in interactive behaviors. See [Smart items](/docs/creator/scene-editor/interactivity/smart-items/) for more details.

![](/docs/creator/images/editor/smart-items.webp)

## The entity tree

On the left margin, you'll see a tree structure with all of the entities in the scene. This includes all of the items you add, as well as a few default entities.


> [!NOTE]
> **💡 Tip**: Everything in a scene is an Entity, they are the basic building blocks of scenes. Items are Entities that have at least a position and a visible shape.


Instead of selecting an item by clicking on it from the 3D view of the scene, you can select it from the tree view. Click the right-mouse button on an entity to reveal more options: you can rename, delete, or duplicate, also create a child entity, turn it into a custom item, or add a component to the entity.

![The entity tree with the right-click menu open on the Bird Fountain entity, listing Rename, Add child, Duplicate, Delete, Create Custom Item and Add component.](/docs/creator/images/editor/entity-tree-context-menu.webp)

### Searching the entity tree

A search box at the top of the entity tree lets you find entities by name. Type part of an entity's name and the tree filters to show only matching entities and their parent hierarchy, so you can see where each match sits in the tree. The search is case-insensitive. Parent entities of matches are automatically expanded so every result is visible.

Press **Escape** or click the clear icon to remove the filter and return to the full tree. If an entity is selected, clearing the search scrolls back to the selected entity so you don't lose your place.

![The entity tree filtered by the search term fount: only the Camera, Scene and Bird Fountain entities remain visible, with the Scene parent expanded to show the match.](/docs/creator/images/editor/entity-tree-search.webp)

### Entity hierarchy

Entities follow a hierarchy that can have as many levels as you want. Establish a parent-child relationship between two entities by dragging one item onto another on the tree. A child entity inherits the position of the parent, so when the parent moves, it carries any children with it. This can be practical while building a scene, for example you can set glasses and plates as children of a table, and then move the table without needing to readjust anything else. It can also be important when interacting with the scene, for items to move together.

![](/docs/creator/images/editor/item-hierarchy.webp)

You can also minimize or expand the children of an entity to keep the view simple, this action has no effect on the scene.

### Special entities

The scene includes a couple of special entities that you can see in the entity tree.

- **Scene**: This refers to the root entity, everything you add in the scene is a child of this entity. You can open it to view [scene settings](/docs/creator/scene-editor/get-started/scene-editor-essentials/#scene-settings).
- **Player**: The player's avatar. You can add special components to this entity that can change gameplay mechanics. You can also drag other entities to be children of the avatar. If an entity is a child of the avatar, its position will be fixed to the player. Use this for example to add a floating marker over the player's head, that follows the player around.
- **Camera**: The player's camera. You can drag other entities to be children of the camera. If an entity is a child of the camera, its position will be fixed on screen. Use this for example to display a gun in a shooter game, that is always in view even if the player points up or down.

### Lock or hide items

You might find it handy to sometimes lock an item, to prevent accidentally selecting and moving it. This is especially useful for background items, like the ground, or a building. To lock an item, look for it on the entity tree on the left, hover over it, and select the lock icon. You can toggle this behavior on and off via that same icon.

You might also want to hide an item that could obstruct your view while placing others. This is especially useful to hide the roof or a building, while working on the interiors. Hidden items are only hidden in the Scene Editor's canvas window, not to players entering the scene. To hide an item, look for it on the entity tree on the left, hover over it, and select the eye icon. You can toggle this behavior on and off via that same icon.

![](/docs/creator/images/editor/hide-lock-item.webp)

## Properties panel

Select an item by clicking on it on the canvas or the entity tree. You'll then see its components displayed on the properties panel, on the right of the screen. Different items have different components that each display specific settings.

![](/docs/creator/images/editor/components-in-editor.webp)

Most non-interactive items have the following components:

- **Transform**: Sets position, rotation, and scale of the item.
- **GLTF**: What 3D model to load.

[Smart items](/docs/creator/scene-editor/interactivity/smart-items/) can include other components.

See [Components](/docs/creator/scene-editor/build/components/) to learn more.

## Scene limitations

Decentraland scenes need to follow certain limitations, to be able to run them one next to another. There is a maximum number of materials, textures, triangles, etc, that is proportional to the number of parcels in the scene. See [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/) for more details.

If the content in your scene exceeds any of these limits, the Scene Editor will notify this on the bottom-left corner.

![](/docs/creator/images/editor/triangle-limit1.webp)

You can expand this menu to view details.

![](/docs/creator/images/editor/triangle-limit2.webp)


> [!NOTE]
> **💡 Tip**: If you're building a Decentraland World, you can always increase the [scene size](/docs/creator/scene-editor/configure/scene-settings/#layout) to increase your limits.


The content in a Decentraland scene must also avoid spilling onto neighbor parcels. If any part of the models in your scene extend beyond the limits, when you open the scene preview you will see these parts cut off. The Scene Editor will mark the entire model in red, but you should only really worry about the parts of the model that extend beyond the scene limits.

![](/docs/creator/images/editor/out-of-bounds.webp)


> [!NOTE]
> **💡 Tip**: If the models you want to display don't fit, you may want to increase the size of your scene. See [scene size](/docs/creator/scene-editor/configure/scene-settings/#layout) to enlarge your scene.


Even if the whole geometry of the 3D model fits in your scene, a model might be marked in red if the model's Bounding Box extends beyond the area. If this is the case, you can ignore the warnings, as the entire model will be displayed correctly. Learn more about [Bounding Boxes](/docs/creator/3d-modeling/meshes/#bounding-boxes).

## Clean up assets

Keep your project tidy by removing assets that are no longer used. Open the **Local Assets** tab at the bottom of the screen and click the brush icon.

![](/docs/creator/images/editor/brush-icon.webp)

A dialog opens listing all assets in your scene and highlights those not referenced by any item.

![](/docs/creator/images/editor/clean-up-assets.webp)

Select the assets you want to delete using the checkboxes, then click **Remove Selected** to permanently remove them.

Deleting an item from the scene does not remove its files. Imported models, textures, or sounds remain in your project until you clean them up, so review unused assets periodically.


> [!WARNING]
> **Important:** If your scene contains code that references assets, some in-use assets may appear as unused. This dialog only detects assets referenced by components in the Creator Hub UI. After you click **Remove Selected**, the files are deleted from the project folder, this action is permanent and can't be undone.


## Scene settings

Click the **Pencil icon** on the top-right of the screen. This opens a series of scene-level properties to edit.

![](/docs/creator/images/editor/pencil-icon.webp)

Here you can configure multiple properties including title and thumbnail, scene size, scene category and age rating, player spawn locations, and feature toggles.

See [Scene Settings](/docs/creator/scene-editor/configure/scene-settings/).

## See also

- See [Smart items](/docs/creator/scene-editor/interactivity/smart-items/) for how to add simple interactivity to your scene.
- See [Combine with code](/docs/creator/scene-editor/code/overview/) for how to edit the code of your scene.
- See [Publish scene](/docs/creator/scene-editor/publish/publish-scene/) for how to publish your scene to Decentraland.
