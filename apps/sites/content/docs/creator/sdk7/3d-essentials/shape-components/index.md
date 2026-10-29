> >-

# Shape Components

Three dimensional scenes in Decentraland are based on the [Entity-Component](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) model, where everything in a scene is an _entity_, and each entity can include _components_ that shape its characteristics and functionality.

The rendered shape of an entity is determined by what component it uses.

![](/docs/creator/images/media/ecs-simple-components-new.webp)

## Use the Scene Editor in Creator Hub

The easiest way to give an entity a shape is to use the Scene Editor. You can add a **Mesh Renderer** component to provide a primitive shape, or a **GLTF** component to reference a 3D model from a file. See [Add Components](/docs/creator/scene-editor/build/components/#add-components).

## Primitive shapes

Several basic shapes, often called _primitives_, can be added to an entity by giving the entity a `MeshRenderer` component.

The following shapes are available. Several shapes include optional additional fields, specific to that shape.

*   **box**:

    Use `MeshRenderer.setBox()`, passing the entity. Pass `uvs` as an additional optional field, to map texture alignment. See [materials](/docs/creator/sdk7/3d-essentials/materials/) for more details.
*   **plane**:

    Use `MeshRenderer.setPlane()`, passing the entity. Pass `uvs` as an additional optional field, to map texture alignment. See [materials](/docs/creator/sdk7/3d-essentials/materials/) for more details.
*   **sphere**:

    Use `MeshRenderer.setSphere()`, passing the entity.
*   **cylinder**:

    Use `MeshRenderer.setCylinder()`, passing the entity. Pass `radiusTop` and `radiusBottom` as additional optional fields, to modify the cylinder.

    TIP: Set either `radiusTop` or `radiusBottom` to 0 to make a cone.

The following example creates a cube:

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)
```

The following example creates a cylinder with a `radiusTop` of 0, which produces a cone:

```ts
const myCone = engine.addEntity()

Transform.create(myCone, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setCylinder(myCone, 1, 0)
```

Primitive shapes don't include materials. To give it a color or a texture, you must assign a [material component](/docs/creator/sdk7/3d-essentials/materials/) to the same entity.

To make a primitive clickable, or to prevent players from walking through it, you must give the entity a _collider_ via a [MeshCollider](/docs/creator/sdk7/3d-essentials/colliders/) component.

To change the shape of an entity that already has a `MeshRenderer` component, run `MeshRenderer.setBox()` or any of the other helper functions and it will overwrite the original shape. There's no need to remove the original `MeshRenderer` or to use the advanced syntax.

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)

// overwrite shape
MeshRenderer.setSphere(myCube)
```


> [!WARNING]
> **📔 Note**: The `MeshRenderer` component must be imported via
> 
> > `import { MeshRenderer } from "@dcl/sdk/ecs"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


## 3D models

For more complex shapes, you can build a 3D model in an external tool like Blender and then import them in _.glTF_ or _.glb_ (binary _.glTF_). [glTF](https://www.khronos.org/gltf) (GL Transmission Format) is an open project by Khronos providing a common, extensible format for 3D assets that is both efficient and highly interoperable with modern web technologies.

To add an external model into a scene, add a `GltfContainer` component to an entity and set its `src` to the path of the glTF file containing the model.

```ts
const houseEntity = engine.addEntity()

GltfContainer.create(houseEntity, {
	src: 'models/House.gltf',
})
```

The `src` field is required, you must give it a value when constructing the component. In the example above, the model is located in a `models` folder at root level of the scene project folder.


> [!NOTE]
> **💡 Tip**: We recommend keeping your models separate in the `assets/scene/models` folder inside your scene.


glTF models can include their own embedded textures, materials, colliders and animations. See [3D models](https://github.com/decentraland/docs-creator/blob/main/creator/3d-modeling/3d-models/README.md) for more information on this. To override the materials of a model, use the [GltfNodeModifiers](/docs/creator/sdk7/3d-essentials/materials/#modify-gltf-materials) component. See [Modify glTF materials](/docs/creator/sdk7/3d-essentials/materials/#modify-gltf-materials) for more details.

To prevent players from walking through a 3D model, or to make a model clickable, you must have a [collider](/docs/creator/sdk7/3d-essentials/colliders/), which may be embedded in the model or provided via a `MeshCollider` component.

Keep in mind that all models, their shaders and their textures must be within the parameters of the [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/).


> [!WARNING]
> **📔 Note**: The `GltfContainer` component must be imported via
> 
> `import { GltfContainer } from "@dcl/sdk/ecs"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


### Pre Loading a 3D Model

In some cases, a custom 3D model is added to the scene but is not used immediately. For example, a custom chair model may only be loaded when the player interacts with another object. In this scenario, the first interaction might take some time until it finishes downloading and loading the model.

To avoid this, use the `AssetLoad.create` method to ensure the asset is downloaded before it is needed.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

For more information, check the [Pre Load Resources](/docs/creator/sdk7/optimizing/pre-load-resources/) documentation.

### Free libraries for 3D models

Instead of building your own 3D models, you can also download them from several free or paid libraries.

To get you started, below is a list of libraries that have free or relatively inexpensive content:

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [Assets from the Builder](https://github.com/decentraland/builder-assets/tree/master/assets)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/) (3D models made primarily for 3D printing, but adaptable to Virtual Worlds)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://www.cgtrader.com/)


> [!WARNING]
> **📔 Note**: Pay attention to the license restrictions that the content you download has.


Note that in several of these sites, you can choose what format to download the model in. Always choose _.glTF_ format if available. If not available, you must convert them to _glTF_ before you can use them in a scene. For that, we recommend importing them into Blender and exporting as _.glTF_ from there.

### Optimize 3D models

To ensure that 3D models in your scene load faster and take up less memory, follow these best practices:

* Save your models in _.glb_ format, which is a lighter version of _.gltf_.
* If you have multiple models that share the same textures, export your models with textures in a separate file. That way multiple models can refer to a single texture file that only needs to be loaded once.
* If your scene has entities that appear and disappear, it might be a good idea to pool these entities and keep them underground, or at a scale of 0. This will help them appear faster, the trade-off is that they will occupy memory when not in use. See [entities and components](/docs/creator/sdk7/architecture/entities-components/#pooling-entities-and-components)

## Stretching a shape

Primitive shapes and 3D models have default dimensions that you can alter by changing the scale in the entity's `Transform` component.

```ts
const primitiveEntity = engine.addEntity()

MeshRenderer.setBox(primitiveEntity)

Transform.create(primitiveEntity, {
	position: { x: 8, y: 1, z: 8 },
	scale: { x: 4, y: 0.5, z: 4 },
})
```

## Make invisible

You can make an entity invisible by giving an entity a `VisibilityComponent`, with its `visible` property set to _false_.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(myEntity)

VisibilityComponent.create(myEntity, { visible: false })
```

The `VisibilityComponent` works the same for entities with primitive shapes and with `GLTFContainer` components.

If an entity is invisible, its collider can block a player's path and/or prevent clicking entities that are behind it, depending on the collision layers assigned to the collider.

### Propagate visibility

You can use the `propagateToChildren` field on the `VisibilityComponent` to apply a configuration to every child in the entity's children tree. If `propagateToChildren` is set to _true_, these settings affect all children on all levels going down. This can save you a lot of tedious work marking each child entity as invisible or visible too.

```ts
// Parent entity (explicitly invisible)
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(parentEntity)
VisibilityComponent.create(parentEntity, { visible: false, propagateToChildren: true })

// Child entity (implicitly invisible because of parent)
const child = engine.addEntity()
Transform.create(child, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity
})
MeshRenderer.setBox(child)
```


> [!WARNING]
> **📔 Note**: If an entity has its own `VisibilityComponent`, this overrides any configuration from parents. 
> 
> If an entity doesn't have its own `VisibilityComponent`, then its visibility is determned by its nearest parent with a `VisibilityComponent` and `propagateToChildren` set to _true_.





## Loading state

If a 3D model is fairly large, it might take some noticeable time to be rendered, this time may vary depending on the player's hardware and many other factors. Sometimes you need to make sure that a model finished loading before you perform another action. For example, if you want to teleport the player to a platform up in the sky, you need to first make sure the platform is fully rendered before moving the player there, or else the player might fall right through the platform.

To check if a 3D model is finished being rendered, check the entity's `GltfContainerLoadingState` component. This component is meant to be read only, and exists on any entity that also has a `GltfContainer`component.

This component has a single property named `currentState`, holding a value from the `LoadingState` enum.

The following example uses a system to periodically check the loading state of an entity's 3D model. If the state is `LoadingState.FINISHED`, you might want to perform custom logic there and end the execution of the system.

```ts
export function main() {
	const meshEntity = engine.addEntity()
	GltfContainer.create(meshEntity, { src: 'models/Monster.glb' })
	engine.addSystem((deltaTime) => {
		const loadingState = GltfContainerLoadingState.getOrNull(meshEntity)
		if (!loadingState) return
		switch (loadingState.currentState) {
			case LoadingState.LOADING:
				console.log('mesh is LOADING')
				break
			case LoadingState.FINISHED:
				console.log('mesh is FINISHED')
				// Perform custom logic
				break
			case LoadingState.FINISHED_WITH_ERROR:
				console.log('mesh is FINISHED BUT MAY HAVE PROBLEMS')
				break
			case LoadingState.UNKNOWN:
				console.log('mesh is in an UNKNOWN STATE')
				break
		}
	})
}
```

## Advanced syntax

The complete syntax for creating a `MeshRenderer` component, without any helpers to simplify it, looks like this:

```ts
MeshRenderer.create(myBox, {
	mesh: {
		$case: 'box',
		box: { uvs: [] },
	},
})

MeshRenderer.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: { uvs: [] },
	},
})

MeshRenderer.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshRenderer.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

This is how the base protocol interprets MeshRenderer components. The helper functions abstract away from this and expose a friendlier syntax, but behind the scenes they output this syntax.

The `$case` field allows you to specify one of the allowed types. Each type supports a different set of parameters. In the example above, the `box` type supports a `uvs` field.

The supported values for `$case` are the following:

* `box`
* `plane`
* `sphere`
* `cylinder`

Depending on the value of `$case`, it's valid to define the object for the corresponding shape, passing any relevant properties.

To add a `MeshRenderer` component to an entity that potentially already has an instance of this component, use `MeshRenderer.createOrReplace()`. The helper functions like `MeshRenderer.setBox()` handle overwriting existing instances of the component, but running `MeshRenderer.create()` on an entity that already has this component returns an error.
