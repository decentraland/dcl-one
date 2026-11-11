> Import your own 3D models, images, sound, etc to use in your scenes.

# Import custom assets

You can import your own 3D models into the Scene Editor. Pick models from a wide selection of free or paid sources on the internet, or to create your own custom models. You can also import other assets like images, sound files, and videos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/UepXpH-k0EI" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=UepXpH-k0EI">Watch on YouTube</a></p>



## Import an asset

To import a 3D model, an image, a sound file, or a video into your scene from your local disk:

1. Drag files directly onto the bottom panel. You can also click the **+ Import Assets** button on the top-left of the bottom panel and select from your local drive.

![](/docs/creator/images/editor/import-assets.webp)

2. Check the model thumbnail and click **Import**. When importing multiple assets, use the arrow buttons to cycle over each asset.

![](/docs/creator/images/editor/import-confirmation.webp)

You can now find your asset in the **Local Assets** tab. Assets are sorted into folders by type: 3D models appear under the _assets/Models_ folder, images under _assets/Images_, sound files under _assets/Audio_, and videos under _assets/Video_.

Items from the built-in free asset packs are stored under _assets/asset-packs/_, and custom items created from the editor under _assets/custom/_. Older scenes may have user imports under _assets/scene/_ instead. All of these paths work in your scene code, just reference whichever folder the asset is in.

- For 3D models, drag the `.glb` or `.gltf` files onto the canvas to add them as items on your scene.
- You can also drag an image, a sound or a video file onto the canvas. The editor creates a ready-made item that already points at your file:
  - A `.png`, `.jpg` or `.jpeg` file becomes an **Image** item.
  - An `.mp3`, `.ogg` or `.wav` file becomes an ambient sound item.
  - An `.mp4` file becomes a **Video Screen** item.

  The new item is named after the file and arrives with the same components and settings as the equivalent item from the asset packs, so you can tweak it from the inspector straight away.
- Any asset can also be dragged onto the fields of an item that is already in the scene. For example you can drag an `.mp3` file onto the _Path_ field of an _Audio Source_ component.


> [!NOTE]
> **💡 Tip**: You can also paste files directly into the project's `assets` folder, from Finder, Explorer or anywhere else. They appear in the **Local Assets** tab on their own, shortly after the copy finishes. If they don't show up, press the **Refresh** button next to the **Import Assets** button.
>
> <img src="/docs/creator/images/editor/refresh-assets.webp" alt="" data-size="original">


### Supported formats

#### Audio

The following Audio formats are supported:

- _.mp3_
- _.wav_
- _.ogg_

#### Image

The following image formats are supported:

- _.png_
- _.jpg_
- _.jpeg_

#### Video

The following video formats are supported:

- _.mp4_

#### 3D Models

The following 3D model formats are supported:

- _.glTF_
- _.glb_

Both can include external texture image files, or external binary (_.bin_) files.

You can convert other formats into these formats with various different editors and tools. See [3D modeling](/docs/creator/3d-modeling/3d-models/) for recommendations and tips.

All materials in the models need to be either _basic material_ or _PBR_, and all textures need to be in sizes that are powers of two (ex: 256, 512). See [Scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/) for details.

Each imported file, of any type, must occupy less than 50 MB to be usable in a scene. Larger files aren't supported.

**Free libraries for 3D models**

Instead of building your own 3D models, you can also download them from several free or paid libraries, or generate them with AI tools. See [Useful Resources](/docs/creator/sdk7/getting-started/useful-resources/) for a list of recommended asset libraries and generative AI tools.


> [!WARNING]
> **📔 Note**: Pay attention to the license restrictions that the content you download has.


Note that in several of these sites, you can choose what format to download the model in. Always choose _.glTF_ or _.glb_ format if available. If not available, you must convert them to _.glTF_ or _.glb_ before you can use them in a scene. For that, we recommend importing them into Blender and exporting them with one of the available _.glTF_ export add-ons.

### Colliders

You might find that when running a preview the player can walk through your imported 3D models. This is likely because the models are missing a _collider mesh_ to define a collision geometry. See [colliders](/docs/creator/3d-modeling/colliders/) for more details and instructions.


> [!NOTE]
> **💡 Tip**: Instead of editing the model to add a _collider mesh_, a simpler alternative is to add an _Invisible wall_ smart item with approximately the same shape to stand in its place.


### Animations

If an imported model includes animations, the first animation that's packed into the model will be played in a loop.

Note that you don't have any control over when the animation starts or stops, or which one is played in case of several animations.

If there are multiple players in the scene, they may be seeing the animation out of sync from each other.

To change this behavior, you can include an **Animator** component. See [Make any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/) for no-code tools to make your item interactive.

## Browse and organize your assets

The **Local Assets** tab of the bottom panel shows every file in your project's `assets` folder.

![The Local Assets tab, with the folder tree on the left, the toolbar with the search field and type filter pills, a breadcrumb reading Assets > Images, two image cards, and the preview pane on the right showing the selected image with its type, size and dimensions](/docs/creator/images/editor/local-assets-preview-pane.webp)
### Find a file

The toolbar above the files helps you find what you need:

- **Folder tree**: show or hide it with the button at the left of the toolbar. Drag its edge to resize it, and double-click the edge to reset its width. You can also expand or collapse every folder at once.
- **Search**: type part of a file name.
- **Type filters**: labelled pills that show only models, images, audio, video, and so on.
- **Breadcrumb**: shows where you are when you're inside a subfolder.

While you search or filter, every folder stays in the tree. Folders with matches show how many they contain, and folders without matches are dimmed. The number of results appears above the files, with a **Clear** link to reset the search.

Use the slider in the panel header to make the file cards bigger or smaller. Image cards show the image itself, and 3D models get a generated thumbnail.

### Preview a file

Select a file to see it in the preview pane on the right of the panel. Hide or show the pane with the eye button in the panel header. The pane works in all three tabs: **Local Assets**, **Custom Items** and **Asset Packs**.

- **3D models**: an orbit viewer, with buttons to auto-rotate and to show the wireframe, a list of the model's animations, and its triangle, mesh, material and texture counts.
- **Images**: shown to fit the pane or at actual size, with their dimensions.
- **Audio and video**: a player, with the file's duration.
- **Avatar animations**: played on an avatar, with play and pause, a timeline you can scrub, and a loop option.
- **Custom items**: their thumbnail, with how many entities and files they contain.
- **Asset pack items**: their **Description** and **Tags**.

If a file can't be shown, for example a damaged image or an audio format the browser can't decode, the pane reads **Preview unavailable** and explains why.

### Rename, copy, move and delete files

Right-click a file to open its menu: **Rename**, **Duplicate**, **Delete**, **Copy Path** and **Copy file name**. Right-click a folder in the tree for the same options plus **New folder**.

To work with several files at once, Ctrl-click (Cmd-click on Mac) to add files to the selection, or Shift-click to select a range. The menu then offers **Copy Paths** and **Copy file names**, and every action applies to the whole selection. To move files, drag them onto a folder.

When you rename or move a file, the items in your scene that use it are updated to the new path. A `.gltf` model takes its textures and `.bin` files along when you copy or move it. Deleting a file that your scene uses asks you to confirm first. Deleting a folder deletes every file inside it.

All of these changes go into the undo history, so press Ctrl+Z (Cmd+Z on Mac) to take one back.


> [!NOTE]
> **💡 Tip**: Double-click a file while the [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/#point-at-an-asset-in-your-prompt) is open to mention it in your next message.



> [!WARNING]
> **📔 Note**: The `assets/scene` folder and the **Built-in animations** folder are read only. You can't rename, move, delete or paste files there. Renames that only change upper and lower case, like `tree.glb` to `Tree.glb`, aren't allowed either: rename the file to a different name and back instead.


### Keyboard shortcuts

Click inside the Local Assets tab first. The shortcuts then act on the files, and stop acting on the 3D view until you click it again.

| Shortcut | Action |
| --- | --- |
| Ctrl/Cmd+A | Select all |
| Esc | Clear the selection |
| Shift+Up / Shift+Down | Extend the selection |
| Delete or Backspace | Delete the selection |
| Ctrl/Cmd+C, Ctrl/Cmd+X, Ctrl/Cmd+V | Copy, cut and paste. A pasted file that clashes with an existing name becomes `name copy.ext`, then `name copy 2.ext`. |
| Ctrl/Cmd+D | Duplicate |
| Up / Down | Move through the folder tree |
| Left / Right | Collapse or expand a folder |
