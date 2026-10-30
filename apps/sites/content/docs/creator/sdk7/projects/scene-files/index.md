> Default files created in a new scene.

# Scene Files

After [creating a new scene](/docs/creator/sdk7/getting-started/sdk-101/), the scene folder will have a series of files with default content.

## Default files in a scene

Scenes include the following files:

* **src/index.ts**: The entry point of the scene.
* **scene.json**: The manifest that contains metadata for the scene.
* **package.json** and **package-lock.json**: Specify the versions of all dependencies of the scene.
* **tsconfig.json**: Typescript configuration file.
* **.dclignore**: Lists the files in your project that will not be uploaded when you publish your scene.
* **assets/scene/main.composite**: Auto-generated file including everything you added and configured visually in the Scene Editor in Creator Hub. It's not meant to be human-readable. It's one example of a [composite](/docs/creator/sdk7/architecture/composites/), a file that describes a tree of entities and components. Scenes created via the CLI instead include a `main.crdt` file at the root level.

### index.ts

This is the entry point to your scene's custom code. You could fit your entire scene's logic into this file, although for clarity in most cases we recommend spreading out your code over several other _.ts_ files.

If you rely only on the Scene Editor and [Smart Items](/docs/creator/scene-editor/interactivity/smart-items/), you won't need to modify this file.

If you intend to write custom code, you'll most likely only need to edit this and other .ts files to create your scene. It contains the code that generates the [entities, components](/docs/creator/sdk7/architecture/entities-components/) and [systems](/docs/creator/sdk7/architecture/systems/) of your scene.

When running the scene, the contents of your `.ts` files are compiled to a single `.js` file, `bin/index.js`. When building for production, this file is also minified.


> [!WARNING]
> **📔 Note**: You can use another tool or language instead of TypeScript, as long as your scripts are contained within a single Javascript file (bin/index.js). All provided type declarations are made in TypeScript, and other languages and transpilers are not officially supported.


### scene.json

The _scene.json_ file is a JSON formatted manifest for a scene in the world. A scene can span a single or multiple LAND parcels. The _scene.json_ manifest describes what objects exist in the scene, a list of any assets needed to render it, contact information for the parcel owner, and security settings.

Most of the fields on the _scene.json_ file can be edited directly in the UI of the Scene Editor. See [Scene Settings](/docs/creator/scene-editor/get-started/scene-editor-essentials/#scene-settings).

![](/docs/creator/images/thumbnail-image.webp)

For more information see [scene metadata](/docs/creator/sdk7/projects/scene-metadata/#metadata).

All of this metadata is optional for previewing the scene locally, but part of it is needed for deploying. You can change this information manually at any time.

### package.json

This file provides information to NPM that allows it to identify the project, as well as handle the project's dependencies. Decentraland scenes require one main package:

* **@dcl/sdk**: The fundamental dependency of the Decentraland SDK, including definitions and types for the engine, components, systems, etc.
* **@dcl/js-runtime**: A series of type declarations that makes the integration of @dcl/sdk smoother.

Your scene may include any number of other packages, for example to include [libraries](https://studios.decentraland.org/resources?sdk_version=SDK7\&resource_type=Library) that can help make the writing of code easier, or enable special functionalities.

### package-lock.json

This file lists the versions of all the other dependencies of the project. These versions are locked, meaning that the compiler will use literally the same minor release listed here.

You can change any package version manually by editing this file.

### tsconfig.json

Directories containing a _tsconfig.json_ file are root directories for TypeScript Projects. The _tsconfig.json_ file specifies the root files and options required to compile your project from TypeScript into JavaScript.

When installing any additional libraries to your scene, an entry should be added automatically to this file. For installing Decentraland utils libraries, it shouldn't be necessary to manually do any changes to this file.

## Recommended file locations

Keep in mind that when you deploy your scene to Decentraland, any assets or external libraries that are needed to use your scene must be either packaged inside the scene folder or available via a remote server.

Anything that is meant to run in the player's client must located inside the scene folder. You shouldn't reference files or libraries that are installed elsewhere in your local machine, because they won't be available to the deployed scene.

We suggest using these folder names consistently for storing the different types of assets that your scene might need:

* 3D models: `assets/scene/models`
* Videos: `assets/scene/videos`
* Sound files: `assets/scene/sounds`
* Image files for textures (except for glTF models): `assets/scene/materials`
* _.ts_ definitions for components `/src/components`
* _.ts_ definitions for systems `/src/systems`


> [!WARNING]
> **📔 Note**: Supporting files for glTF models, like their texture image files or _.bin_ files, should always be placed in the same folder as the model's _.gltf_ or _.glb_ file.



> [!WARNING]
> **📔 Note**: We recommend using always lower case names for all folders and file names, to avoid possible issues.


When importing any assets via the Scene Editor, they are added automatically inside the `assets/scene` folder. When using any of the default assets in the Asset Packs of the Scene Editor, their files are added to the `assets/builder` folder.

## The dclignore file

All scenes include a _.dclignore_ file, this file specifies what files in the scene folder to ignore when deploying a scene to Decentraland.

For example, you might like to keep the Blender files for the 3D models in your scene inside the scene folder, but you want to prevent those files from being deployed to Decentraland. In that case, you could add `*.blend` to _.dclignore_ to ignore all files with that extension.

| What to ignore | Example     | Description                                                                             |
| -------------- | ----------- | --------------------------------------------------------------------------------------- |
| Specific files | `BACKUP.ts` | Ignores a specific file                                                                 |
| Folders        | `drafts/`   | Ignores entire contents of a folder and its subfolders                                  |
| Extensions     | `*.blend`   | Ignores all files with a given extension                                                |
| Name sections  | `test*`     | Ignores all files with names that match the query. In this case, that start with _test_ |
