> Import your own 3D models, images, sound, etc to use in your scenes.

# Import custom assets

You can import your own 3D models into the Scene Editor. Pick models from a wide selection of free or paid sources on the internet, or to create your own custom models. You can also import other assets like images, sound files, and videos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/UepXpH-k0EI" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=UepXpH-k0EI">Watch on YouTube</a></p>



## Import an asset

To import a 3D model, an image, a sound file, or a video into your scene from your local disk:

1. Drag files directly onto the bottom panel. You can also click the **+ Import Assets** button on the top-left of the bottom panel and select from your local drive.

![](/docs/creator/images/editor/import-assets.webp)

2. Check the model thumbnail and click **Import**. When importing multiple assets, use the arrow buttons to cycle over each asset.

![](/docs/creator/images/editor/import-confirmation.webp)

You can now find your asset in the **Local Assets** tab. Assets are sorted into folders by type: 3D models appear under the _assets/Models_ folder, images under _assets/Images_, sound files under _assets/Audio_, and videos under _assets/Video_.

Items from the built-in free asset packs are stored under _assets/asset-packs/_, and custom items created from the editor under _assets/custom/_. Older scenes may have user imports under _assets/scene/_ instead. All of these paths work in your scene code, just reference whichever folder the asset is in.

- For 3D models, drag the `.glb` or `.gltf` files onto the canvas to add them as items on your scene.
- Other kinds of assets like images and sound files can be dragged onto the fields of an item. For example you can drag an `.mp3` file onto the _Path_ field of an _Audio Source_ component.


> [!NOTE]
> **💡 Tip**: You can also paste files directly into the project's `assets` folder, from Finder, Explorer or anywhere else. They appear in the **Local Assets** tab on their own, shortly after the copy finishes. If they don't show up, press the **Refresh** button next to the **Import Assets** button.
> 
> <img src="/docs/creator/images/editor/refresh-assets.webp" alt="" data-size="original">


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
