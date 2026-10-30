> Learn about how textures works in Decentraland

# Textures

Textures are a key part of the 3D art pipeline to achieve the look and feel you want for your scene. In this section you will find everything you need to know to create your textures, limitations, nodes in Blender and optimizing them to perform as its best!


> [!NOTE]
> **💡 Tip**: Install the [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). It includes several handy functions to help you edit and export 3D models, wearables, and emotes.


## Limitations

### Texture Size Constraints

Currently the Decentraland Explorer compress the textures at a maximun of 1024px for optimization purpeses using an Asset Bundle Converter after the scene is uplaoded to the content servers. Be sure to take this limitations into account when creating your assets!

Another important point to take into account is that textures should be always power of two, any textures that are not following this specification may bring issues when rendering the scene.

Texture sizes must use width and height numbers (in pixels) that match the following numbers:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> This sequence is made up of powers of two: `f(x) = 2 ^ x` . **1024 is the maximum number we allow for a texture size.** This is a fairly common requirement among other rendering engines, it's there due internal optimizations of the graphics processors.

The width and height don't need to have the same number, but they both need to belong to this sequence.

**The recommended size for textures is 1024x1024**, we have found this to be the optimal size to be transported through domestic networks and to provide reasonable loading/quality experiences.

Examples of other valid sizes:

```
32x32px
64x32px
512x256px
512x512px
1024x1024px
```

On another hand there is a limit for textures per parcel:

```
log2(n+1) x 10 Amount of textures per parcel. It includes textures imported as part of models.
```

## UVMapping

UVmapping is the process of unwrapping the faces of your 3d model into a 2D coordinate that will be used later to add the different maps to your assets. It's a key part of the creation process. Doing a correct unwrap of your models is a key factor to squeeze the resolution of your models and also it will organize your maps to be flexible for modification.

To know more about UV Unwrapping you can see this awesome video made by the Blender Foundation:

[![Video Preview](/docs/_remote/b25b76a5b0e4c1d0cec817e8.webp)](https://youtu.be/Y7M-B6xnaEM?si=qMnWTMsXxC-vxZAH)

There is another great intermidiate level tutorial for unwrapping UVs made by \[Blender Guru] (https://www.youtube.com/@blenderguru) that explains how to unwrap a more complex model:

[![Video Preview](/docs/_remote/2b79a21fed4d0ba2abc23bfa.webp)](https://youtu.be/scPSP_U858k?si=Uw0xHbv9jtqVstDS)

## Maps

In the [materials section](/docs/creator/3d-modeling/materials/) we explained how Decentraland works with the PBR shaders. In this section we're going to show you how each texture map modify the shader and the look and feel of the 3D object that later is going to be exported to the world.

## Diffuse Maps

This is the base color of the object's surface. Having a balanced color palette between your models is key to achieve a cohesive look and feel for your experience.

![](/docs/.gitbook/assets/58-difusemap.webp)

Here it is a cool free palette generator in case you need it!

* **Coolors:** https://coolors.co/

Or some palette inspiration provided by awesome films:

* **Movies in Color:** https://moviesincolor.com/

## Metallic Maps

In a Metallic map, the grayscale map represents the grade of metalness an object posess, being white full metallic and black non metallic. In the following example we see how the light affects the model and how interacts with the environment.

![](/docs/.gitbook/assets/64-metallic-map.webp)

_In the image we can see how the dark parts of the texture affect the model. The dark stripes are opaque, while the white stripe is beahving under the metallic shader properties and gray being a mixture between both._

## Roughness Maps

In a roughness map, darker areas correspond to smoother surfaces, while brighter areas correspond to rough surfaces. This grayscale representation is used by rendering engines to determine how light should be scattered or reflected at different points on the surface.

![](/docs/.gitbook/assets/48-roughness.webp) ![](/docs/.gitbook/assets/49-roughness.webp)

## Transparent Maps

### Alpha Clip

Alpha Clip in Blender, when used with a texture, involves using the alpha channel of the texture to determine which parts of the material should be visible. Pixels with alpha values above a specified threshold are shown, while those below the threshold are discarded, creating a cutout effect based on the texture's transparency information.

In the following example we used a material with alpha clip, using 2 textures, the diffuse color and the alpha texture in black and white connecting the color to the alpha channel.

![](/docs/.gitbook/assets/33-alpha-clip-leave.webp) ![](/docs/.gitbook/assets/33-alpha-clip-mask.webp)

![](/docs/.gitbook/assets/33-alpha-blender.webp)

_As a result we can see the areas painted in black are discarted while the white area are being rendered_

### Alpha Blend

_Alpha Blend_ allows you to pick intermediate values per region.

Alpha Blend in Blender, when used with a texture, involves blending the transparent and opaque areas of the texture based on its alpha channel. This allows the texture to smoothly combine both visible and see-through portions, interacting naturally with the background or other objects in the scene.

![](/docs/.gitbook/assets/33-alpha-blend.webp)

_While Alpha Clip render values being 0 or 1 (in a specific threshold) Alpha Blend interpolates the values between 0 and 1. In the example above the alpha blend material shows the complete gradient transition while the alpha clip excludes part of the texture set by the clip threshold_


> [!WARNING]
> **🔥Optimization Tip🔥**
> 
> * Take into account that transparent textures (RGB+A) are always more expensive in termns of performance that using a grayscale value for transparencies.
> * Transparency is always an expensive operation when rendering the scene. Try always to keep the transparencies at minimun and use Alpha Blend only when it's necessary, otherwise Alpha Clip is preferred rather than Alpha Blend.


## Emissive Maps

An emissive map is a type of texture map used to control the self-illumination or the emitted light of a surface in a 3D scene. It's a component of the shader that determines how much light (and color of light) a particular part of a 3D model emits, independently of external light sources. Emissive maps are commonly used to simulate materials or objects that appear to emit their own light.

![](/docs/.gitbook/assets/57-emissive-map.webp)

_In this example we can see the use of an emissive map combined with emissive strenght in an environment that uses glow postprocessing to test approxivamtely how the emissive behavies in world._

## Normal Maps

A normal map is a type of texture used in 3D graphics to simulate fine surface details and create the illusion of complex geometry without actually altering the underlying geometry of a model. It's commonly used to enhance the realism of low-poly models by adding the appearance of bumps, crevices, and other surface irregularities. It also allows you to keep the object themselves lighter, as lots of details can be provided on the normal map layer instead of complex geometry.

![](/docs/.gitbook/assets/51-normal-map.webp) ![](/docs/.gitbook/assets/50-normal.webp)

To add a normal map to your material using the _Shader Editor_, you will need to connect the _**Normal Map**_ node between the texture and the _Principled BSDF_ shader.

![](/docs/.gitbook/assets/56-normal-map.webp)


> [!WARNING]
> **⚠️Important⚠️:** Never use a texture as albedo and normal at the same time because can create issues when rendering the scene.


## Optimizing Textures

The process of optimization textures brings a lot of benefits when rendering the scene in the explorer but also it's a good way to keep the style of your scene consistent and more flexible in the design process. Some of these benefits are:

* Having optimized textures in size and compression will run the scene much smoother and faster, making it faster to download and easier to render (specially for players that have a slow internet connection).
* It reduces the amount of memory and processing power to render your experience, resulting in a better user experience for your players.
* It saves storage from the community content servers.
* Using **Texture Atlases** and/or **Trim Sheets** will give you more flexibility to iterate the creation of your scene and style consistensy between the objects. Using these techniques you can easily swap textures, adjust colors or patterns instead of doing it individually for each model.
* Sharing textures across models allows to have less textures per scene, reducing the draw calls in game drastically. If you're working with glbs (with embebbed textures) you will find an extruder in the following guidelines to extract the textures from it, redirecting the models to use the same texture.

## Shared Textures Between glTFs Models

A wise and common practice for optimizing your scene is sharing textures and materials between models across the scene. Doing this will reduce the draw calls drastically and your Decentraland scene will run much smoother.

The following tool based on [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)**,** offers some optimizations that will make 3D models lighter and faster to download for players in your scene.

**Mac:** [MAC GLB Extractor](https://github.com/decentraland/docs-creator/blob/main/images/3d-models-and-animations/glb-extractor/texture_extractor.sh)

**PC:** [PC GLB Extractor](https://github.com/decentraland/docs-creator/blob/main/images/3d-models-and-animations/glb-extractor/glb_texture_extract.bat)

It converts .gltf format into .glb, which is binary and so occupies a lot less. It also places texture files outside the 3D model, which allows you to use the same texture on multiple models.

> 📔 Note: .glb format by default always has textures embedded in the file. The engine can’t recognize two embedded textures as the same, they need to be external files that share a same hash.

### How To Use GLB Texture Extractor

In this example scene, we have a simple sci-fi scene in Blender.

![](/docs/.gitbook/assets/01-scene-base.webp)

This scene contains the base environment for the static models but there are also two other assets, a droid and a spaceship that we want to export separately in order to move them later by code, so they can interact with the players. In this case we used 4 textures (one for the floor, one atlas color map for most of the assets, one emissive and a UI sci-fi texture for the panels)

![](/docs/.gitbook/assets/02-model-01.webp) ![](/docs/.gitbook/assets/03-model-02.webp)

Once we export all of these assets to the models folder we have 3 models, the environment static scene, the spaceship and the droid.

![](/docs/.gitbook/assets/04-models-folder.webp)

But we have a problem, the .glb files have the textures embedded in them, so if you are exporting different assets that reuse the textures, these would be duplicated each time there is a new asset in the folder. To avoid having duplicated textures we can use this helpful tool.


> [!WARNING]
> ⚠️ IMPORTANT NOTE: Before using the tool do a BACKUP of your models, just in case something goes wrong!!!


**On Mac**

Once you have exported all the assets to your models folder you can drag the script file to it.

![](/docs/.gitbook/assets/05-texture-extractor.webp)

1 - Open the folder in the terminal by dragging the folder to it.

![](/docs/.gitbook/assets/06-move-to-terminal.webp)

2 - Drag the texture-extractor.sh to the terminal.

![](/docs/.gitbook/assets/06-terminal.webp)

3 - Run the command. It may take some seconds to process all the assets. After that, you will see something like this:

![](/docs/.gitbook/assets/07-run-terminal.webp)

4 - Go to your models folder and there will be one new folder called "out" in which you will see your new assets with the textures extracted.

![](/docs/.gitbook/assets/08-out-folder.webp)

5 - Replace all the assets for the new ones! Also once you finished erase the "texture-extractor.sh" and the empty folder "out".

![](/docs/.gitbook/assets/09-replace-assets.webp)

**On Windows**

Once you have exported all the assets to your models folder you can drag the script file into it.

![](/docs/.gitbook/assets/10-windows-01.webp)

1 - Double click on glb\_texture\_extract.bat to extract the files. If Windows warns you about unrecognized app, go to **More Info** and then **Run anyway**.

![](/docs/.gitbook/assets/11-windows-02.webp) ![](/docs/.gitbook/assets/12-windows-03.webp)

2 - The script will generate a folder called out , there you will fid all new .glb files with its extracted textures.

![](/docs/.gitbook/assets/13-windows-03.webp)

5 - Replace all assets with the new ones. Once finished, delete the "texture-extractor.bat" script and the empty "out" folder.

If you follow all the steps your scene will be much faster now and the assets will share the same texture! When we work with several assets and big scenes the improvement is quite noticeable! With this tool you can save quite a lot of megabytes of information!


> [!WARNING]
> ⚠️ After completing this step, it is crucial to check that every texture follows the guidelines. If the textures do not follow the guidelines or are too heavy, optimize them to be lighter before deploying. The recommended texture size for performant scenes is 1024x1024. Also check resolution, images with more than 72 DPI would affect performance and won’t make the image look any better.


## Texture Atlas/ Trim Sheets

A texture atlas is a single image file that contains data from several smaller images packed together. Rather than having one texture for each mesh, several meshes share a larger texture.

You can create a texture atlas before making the asset, which means that the asset is UV-unwrapped according to the texture atlas. This requires some early planning when creating the texture.

Alternatively, you can create the texture atlas after the asset is finished by merging textures in painting software. However, this also means that the UV islands must be rearranged according to the texture.

Below is an image showing several 3D objects that use one texture set:

![](/docs/.gitbook/assets/37-atlas-texture.webp) ![](/docs/.gitbook/assets/38-uv-atlas.webp)

Another way to do Atlases is the use of Trim Sheets, a common technique in 3D modeling and game development that big studios and game industry use to ensure visual consistency and efficent art pipeline. Trim Sheets is the usage of a single image or texture that contains multiple small details or elements that can be applied to different parts of a model. This technique is very useful when creating large scenes and it needs to be considered in the first stage of the art pipeline.

![](/docs/.gitbook/assets/39-trim-sheets.webp)

_A collage example of a Trim Sheet that uses a diffuse texture, normal and emissive._

There is a great [tutorial](https://www.artstation.com/blogs/jennifermcgarry/yd4Q/jenns-guide-to-trim-sheets) by [Jennifer McGarry](https://www.artstation.com/jennifermcgarry/blog) that explain the use of Trim Sheets using Blender!

## Texture Naming

**It's crucial to name our textures correctly.** Having a correct naming for the texture will:

* Make your art pipeline more efficient, flexible, organized, easy to target and modify if it's necessary.
* Avoid of to overlap textures with the same name using the texture extractor or others tools.
* Avoid issues of overlapping textures when using Asset Bundles.
* More efficient way to pain point issues when analasing the scene.

**How to properly name your Textures:**

* **Asset name** should clearly represent what the texture is.
* Textures name should start with the prefix `T_`.
* Texture name should end with the suffix that defines the texture type:
  * `_D` - **Diffuse/ Color Map**
  * `_A` - **Alpha Texture**
  * `_MT` - **Metallic**
  * `_R` - **Roughness**
  * `_N` **- Normal Map**
  * `_E` - **Emission**

Example: If it's a diffuse map of brick texture for a wall, the name `T_BrickWall_D` could be appropriate. If it's a Normal Map of the same asset the name could be `T_BrickWall_N`

**Examples:**

* 🟢 **Prefer** starting texture name with - `T_Parquet_D`, `T_Floor_R`, `T_Pipes_MT`,
* 🔴 **Avoid** starting texture name with - `Image_`, `sprite_`,`Untitled`

## Optional Tools

There are lots of addons and externals tools that facilitate the work when creating assets to make the pipeline faster and more efficient, some of they are free and some to purchase, to name a few:

### UVTools

* **UV Packer (Free)**:https://www.uv-packer.com/blender/
* **Zen UV:** https://blendermarket.com/products/zen-uv
* **Uvpackmaster 3:** https://blendermarket.com/products/uvpackmaster

### Image Compressors

There are several image compressors online that you can use in order to make your textures lighter. To name a few:

* **CompressPNG:** https://compresspng.com/
* **TinyPNG:** https://tinypng.com/
* **FreeConvert:** https://www.freeconvert.com/
