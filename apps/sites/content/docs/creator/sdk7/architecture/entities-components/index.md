> Learn the essentials about entities and components in a Decentraland scene

# Entities & Components

Decentraland scenes are built around [_entities_, _components_ and _systems_](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system). This is a common pattern used in the architecture of several game engines, that allows for easy composability and scalability.

![](/docs/creator/images/media/ecs-big-picture.webp)

## Overview

_Entities_ are the basic unit for building everything in Decentraland scenes. All visible and invisible 3D objects and audio players in your scene will each be an entity. An entity is nothing more than an id, that can be referenced by components. The entity itself has no properties or methods of its own, it simply serves to group several components together.

_Components_ define the traits of an entity. For example, a `Transform` component stores the entity's coordinates, rotation and scale. A `MeshRenderer` component gives the entity a visible shape (like a cube or a sphere) when rendered in the scene, a `Material` component gives the entity a color or texture. You can also create custom components to serve your scene's required data, for example a custom `health` could store an entity's remaining health value, and add it to entities that represent non-player enemies in a game.

If you're familiar with web development, think of entities as the equivalent of _Elements_ in a _DOM_ tree, and of components as _attributes_ of those elements.

In the [Scene Editor in Creator Hub](/docs/creator/scene-editor/get-started/about-editor/), you can view the components that belong to an entity by selecting it.

![](/docs/creator/images/editor/components-example.webp)


> [!WARNING]
> **📔 Note**: In previous versions of the SDK, Entities were _objects_ that were instanced, and could be extended to add functions. As of version 7.0 of the SDK, entities are only an ID. This structure better fits the principles of [data oriented programming](/docs/creator/sdk7/architecture/data-oriented-programming/) and can help in the scene's performance.


![](/docs/creator/images/media/ecs-components-new.webp)

Components like `Transform`, `Material` or any of the _shape_ components are closely tied in with the rendering of the scene. If the values in these components change, that alone is enough for the engine to change how the scene is rendered in the next frame.

The engine is the part of the scene that sits in the middle and manages all of the other parts. It determines what entities are rendered and how players interact with them. It also coordinates what functions from [systems](/docs/creator/sdk7/architecture/systems/) are executed and when.

Components are meant to store data about their referenced entity. They can only store this data, they can't modify this data themselves. All changes to the values in the components are carried out by [Systems](/docs/creator/sdk7/architecture/systems/). Systems are completely decoupled from the components and entities themselves. Entities and components are agnostic to what _systems_ are acting upon them.

## Syntax for entities and components

The example below shows some basic operations for declaring, and configuring basic entities and components.

```ts
export function main() {
	// Create an entity
	const door = engine.addEntity()

	// Give the entity a position via a transform component
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// Give the entity a visible shape via a GltfContainer component
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```


> [!WARNING]
> **📔 Note**: In previous versions of the SDK, it was necessary to manually add an entity to the engine to start rendering it. As of version 7 of the SDK, entities are implicitly added to the engine as soon as they are assigned a component.


When a component is created, it's always assigned to a parent entity. The component's values then affect the entity.


> [!NOTE]
> **💡 Tip**: Instead of creating entities one by one, you can spawn a whole tree of entities and components at once from a [composite](/docs/creator/sdk7/architecture/composites/) file.


## Remove entities

To remove an entity from the engine, use `engine.removeEntity()`. This function returns a `boolean`: `true` if the entity was removed, `false` if the removal was refused.

```ts
export function main() {
	// Create an entity
	const door = engine.addEntity()

	// Give the entity a visible shape via a GltfContainer component
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})

	// Remove entity
	const removed = engine.removeEntity(door)
	console.log('Entity removed:', removed) // true
}
```

If a removed entity has any child entities, these change their parent back to the default `engine.RootEntity` entity, which is positioned at the scene base position, with a scale of _1_.

### Renderer-reserved entities

Some entity ids are reserved by the renderer for remote player avatars. You cannot remove these entities. If you call `engine.removeEntity()` on a renderer-reserved entity, it returns `false` and leaves all components untouched.

The [named reserved entities](#reserved-entities) (`engine.RootEntity`, `engine.PlayerEntity`, `engine.CameraEntity`) are a special case: `engine.removeEntity()` still returns `false` for these (their ids are never released), but their components **are** purged. This means you can clear your own components from them (for example, removing an `InputModifier` from `engine.PlayerEntity`), even though the entity id itself is never freed.

```ts
// Attempting to remove a reserved entity
const result = engine.removeEntity(engine.PlayerEntity)
console.log(result) // false — the entity id is not released
// But the scene's own components on PlayerEntity are purged
```

You can check the return value when removing entities to handle edge cases:

```ts
if (!engine.removeEntity(someEntity)) {
	console.log('Entity could not be removed (reserved)')
}
```

### Remove an entity with its children

To remove an entity and also all of its children (and any children of its children, recursively), use the `removeEntityWithChildren()` helper.

```ts
export function main() {
	// Create parent entity
	const door = engine.addEntity()

	// Create child entity
	const doorKnob = engine.addEntity()

	// Give the entities a visible shape
	GltfContainer.create(door, {
		src: 'models/door.glb',
	})
	GltfContainer.create(doorKnob, {
		src: 'models/doorKnob.glb',
	})

	// Parent
	Transform.create(doorKnob, {
		parent: door,
	})

	// Remove both parent and children
	removeEntityWithChildren(engine, door)
}
```


> [!WARNING]
> **Note:** If a renderer-reserved entity sits anywhere in the tree, `removeEntityWithChildren` removes all the other descendants but leaves the reserved entity in place. The reserved entity's `Transform.parent` will point at a removed entity. This only happens if your scene parents a reserved entity under a scene entity, which is uncommon.



> [!NOTE]
> **💡 Tip**: Instead of removing an entity from the engine, in some cases it might be better to make it invisible, in case you want to be able to load it again without any delay. See [Make invisible](/docs/creator/sdk7/3d-essentials/shape-components/#make-invisible)


### Removing entities behind the scenes

An entity is just an id that is referenced by its components. So when removing an entity you're really removing each of the components that reference this entity. If you manually remove all of the components of an entity, it will look the same to the player as doing `engine.removeEntity()`. However, `engine.removeEntity()` also performs some extra internal bookkeeping, marking the entity id as no longer in use, so it's always the recommended way to remove an entity.

## Nested entities

An entity can have other entities as children. Thanks to this, we can arrange entities into trees, just like the HTML of a webpage.

![](/docs/creator/images/media/ecs-nested-entities-new.webp)

To set an entity as the parent of another, the child entity must have a `Transform` component. You can then set the `parent` field with a reference to the parent entity.

```ts
export function main() {
	// Create entities
	const parentEntity = engine.addEntity()

	const childEntity = engine.addEntity()

	// Set parent
	Transform.create(childEntity, {
		parent: parentEntity,
	})
}
```

Once a parent is assigned, it can be read off the child entity from the `parent` field on its `Transform` component.

```ts
// Get parent from an entity
const parent = Transform.get(childEntity).parent
```

If a parent entity has a `Transform` component that affects its position, scale or rotation, its children entities are also affected. Any position or rotation values are added, any scale values are multiplied.

If either the parent or child entity doesn't have a `Transform` component, the following default values are used.

* For **position**, the parent's center is _0, 0, 0_
* For **rotation** the parent's rotation is the quaternion _0, 0, 0, 1_ (equivalent to the Euler angles _0, 0, 0_)
* For **scale**, the parent is considered to have a size of _1_. Any resizing of the parent affects scale and position in proportion.

Entities with no shape component are invisible in the scene. These can be used as wrappers to handle and position multiple entities as a group.

To separate a child entity from its parent, you can assign the entity's parent to `engine.RootEntity`.

```ts
const mutableChildTransform = Transform.getMutable(childEntity)
mutableChildTransform.parent = engine.RootEntity
```


> [!WARNING]
> **📔 Note**: When dealing with nested entities that are synced with other players, use the `parentEntity()` function instead of the `parent` entity in the Transform. See [Parented entities](/docs/creator/sdk7/networking/serverless-multiplayer/#parented-entities)


In the Scene Editor, you can see the entire hierarchy of nested entities in your scene on the left-side panel.

![](/docs/creator/images/editor/entity-tree-example.webp)

## Get an entity by ID

Every entity in your scene has a unique number _id_. You can retrieve a component that refers to a specific entity from the engine based on this ID.

```typescript
// fetch a Transform component
Transform.get(1000 as Entity)
```


> [!WARNING]
> **📔 Note**: The entity ids between _0_ and _511_ are reserved by the engine for fixed entities, like the player avatar, the base scene, etc.


For example, if a player's click or a [raycast](/docs/creator/sdk7/interactivity/raycasting/) hits an entity, this will return the id of the hit entity, and you can use the command above to fetch the Transform component of the entity that matches that id. You can also fetch any other component of that entity in the same way.

## Get an entity by name

When adding entities via drag-and-drop in the Scene Editor, each entity has a unique name. Use the `engine.getEntityOrNullByName()` function to reference one of these entities from your code. Pass the entity's name as a string, as written on the Scene Editor's UI, in the tree view on the left.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```


> [!WARNING]
> **📔 Note**: Make sure you only use `engine.getEntityOrNullByName()` inside the `main()` function, in functions that run after `main()`, or in a system. If used outside one of those contexts, the entities created in the Scene Editor UI may not yet be instanced.


You're free to perform any action on an entity fetched via this method, like add or remove components, modify values of existing components, or remove the entity from the engine.

```ts
function main() {
	// fetch entity
	const door = engine.getEntityOrNullByName('door-3')
	// verify that the entity exists
	if (door) {
		// add a pointer events callback
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open' },
			},
			function () {
				// open door
			}
		)
	}
}
```

All the entities added via the Scene Editor UI have a `Name` component, you can iterate over all of them like this:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## Add or replace a component

Each entity can only have one component of a given kind. For example, if you attempt to assign a Transform to an entity that already has one, this will cause an error.

To prevent this error, you can use `.createOrReplace` instead of `.create`. This command overwrites any existing components of the same kind if they exists, otherwise it creates a new component just like `.create`.

```ts
Transform.createOrReplace(door, {
	position: Vector3.create(5, 1, 5),
})
```


> [!WARNING]
> **📔 Note**: Since `.createOrReplace` runs an additional check before creating the component, it's always more performant to use `.create`. If you're sure that the entity doesn't already have a component like the one you're adding, use `.create`.


## Access a component from an entity

You can access components of an entity by using the entity's `.get()` or the `getMutable()` functions.

```ts
export function main() {
	// Create entity
	const box = engine.addEntity()

	// Create and add component to that entity
	Transform.create(box)

	// Get read-only version of component
	let transform = Transform.get(box)

	// Get mutable version of component
	let transform = Transform.getMutable(box)
}
```

The `get()` function fetches a read-only reference to the component. You cannot change any values from this reference of the component.

If you wish to change the values of the component, use the `getMutable()` function instead. If you change the values in the mutable version of the component, you're directly affecting the entity that component belongs to.

See [mutable data](/docs/creator/sdk7/programming-patterns/mutable-data/) for more details.


> [!WARNING]
> **📔 Note**: Only use `getMutable()` if you're actually going to make changes to the component's values. Otherwise, always use `get()`. This practice follows the principles of [data oriented programming](/docs/creator/sdk7/architecture/data-oriented-programming/), and can significantly help in the scene's performance.


```ts
// Get mutable version of component
let transform = Transform.getMutable(box)

// change a value of the component
transform.scale.x = 5
```

The example above directly modifies the value of the _x_ scale on the Transform component.

If you're not entirely sure if the entity does have the component you're trying to retrieve, use `getOrNull()` or `getMutableOrNull()`.


> [!WARNING]
> **📔 Note**: Avoid using `getOrNull()` or `getMutableOrNull()` when possible, as these functions involve additional checks that and are therefore less efficient than `.get()` and `getMutable()`.


```ts
//  getOrNull
const transformOrNull = Transform.getOrNull(myEntity)

//  getMutableOrNull
const mutableTransformOrNull = Transform.getMutableOrNull(myEntity)
```

If the component you're trying to retrieve doesn't exist in the entity:

* `get()` and `getMutable()` returns an error.
* `getOrNull()` and `getMutableOrNull()` returns `Null`.

## Remove a component from an entity

To remove a component from an entity, use the entity's `deleteFrom()` method of the component type.

```ts
Transform.deleteFrom(myEntity)
```

If you attempt to remove a component that doesn't exist in the entity, this action won't raise any errors.


> [!WARNING]
> **📔 Note**: To remove all the components of an entity at once, see [this section](/docs/creator/sdk7/architecture/entities-components/#remove-entities)


## Check for a component

You can check if an entity owns an instance of a certain component by using the `has()` function. This function returns _true_ if the component is present, and _false_ if it's not. This can be very handy for using in conditional logic in your scene.

```ts
const hasTransform = Transform.has(myEntity)
```


> [!NOTE]
> **💡 Tip**: You can also [query components](/docs/creator/sdk7/architecture/querying-components/) to fetch a full list of components that hold a specific component, or a specific set of components. Do not iterate over all entities in the scene manually to check each with a `has()`, that approach is a lot less efficient.


## Check for changes on a component

Use the `onChange` function to run a callback function any time that the values of the component change for a given entity. This works on any component, and is a great shortcut for helping keep your code readable.

The callback function can include an input parameter that contains the new state of the component.

```ts
Transform.onChange(cubeEntity, (newTransform) => {
	if (!newTransform) return
	console.log(
		'Cube position changed: ',
		newTransform.position,
		newTransform.rotation
	)
})

VisibilityComponent.onChange(cubeEntity, (newVisibilityComponent) => {
	if (!newVisibilityComponent) return
	console.log('Cube visibility changed: ', newVisibilityComponent.visible)
})
```

If the component is removed from the entity, then the function is called with an input of `undefined`.


> [!NOTE]
> **💡 Tip**: The `.onChange()` function works both with native components of the SDK and with [custom components](/docs/creator/sdk7/architecture/custom-components/) defined by the creator.


## Get child entities

To access all the entities that are direct children of a parent entity, use `getEntitiesWithParent`. It takes as its arguments the `engine` and the `parent` entity and returns a list of all the entities that have that particular entity as its parent. Note that it only returns direct children, not children of children.

```ts
import { getEntitiesWithParent } from '@dcl/sdk/ecs'

const children = getEntitiesWithParent(engine, myEntity)
for (const child of children) {
   // process each child entity
}
```

To instead access all the descendants of an entity, regardless of how deeply nested they are, use the function `getComponentEntityTree()`. Instead of manually traversing the hierarchy level by level, this function returns a flat list of all descendants that is easy to iterate over. It also filters to only entities that have a given component or list of components.


```ts
import { getComponentEntityTree } from '@dcl/sdk/ecs'

export function main() {
	// Create a parent entity with nested children
	const parentEntity = engine.addEntity()
	Transform.create(parentEntity, {
		position: Vector3.create(8, 0, 8),
	})

	// ... assume the parent has multiple children and grandchildren

	// Iterate over all descendants of the parent entity
	for (const descendantEntity of getComponentEntityTree(
		engine,
		parentEntity,
		Transform
	)) {
		// Access each descendant entity
		const transform = Transform.get(descendantEntity)
		console.log('Descendant position:', transform.position)
	}
}
```

The `getComponentEntityTree` function takes three parameters:

* `engine`: The engine instance running the entities
* `entity`: The root entity to start from
* `component`: The component to filter by (typically `Transform` for spatial hierarchies)

The function returns a generator that yields each descendant entity in the tree structure. Only entities that have the specified component will be included in the results.

You can combine this with other component checks to find specific entities in your hierarchy:

```ts
// Find all descendants with a specific name
for (const descendantEntity of getComponentEntityTree(
	engine,
	parentEntity,
	Transform
)) {
	const name = Name.getOrNull(descendantEntity)
	if (name && name.value === 'targetEntity') {
		console.log('Found target entity:', descendantEntity)
	}
}
```


## Reserved entities

Certain entity ids are reserved for special entities that exist in every scene. They can be accessed via the following aliases:

* `engine.RootEntity`
* `engine.PlayerEntity`
* `engine.CameraEntity`


> [!WARNING]
> **📔 Note**: Avoid referring to these entities before they are initialized. To avoid this problem, refer to these entities in the `main()` function, or in a system.


## The root entity

All entities in the scene are children of the `engine.RootEntity`, directly or indirectly.

This entity lacks a Transform component, but it's used to handle several components that represent more global settings, like [skybox control](/docs/creator/sdk7/interactivity/skybox-control/), [cursor position](/docs/creator/sdk7/interactivity/user-data/#check-the-players-cursor-position), or [screen dimensions](/docs/creator/sdk7/2d-ui/ui-positioning/#responsive-ui-size).

## The player entity

The `engine.PlayerEntity` entity represents the player's avatar.

Fetch the player's `Transform` component to get the player's current position and rotation, see [user data](/docs/creator/sdk7/interactivity/user-data/). The player's Transform is read-only, to modify it use the `movePlayerTo()` function, [learn more](/docs/creator/sdk7/interactivity/avatars/move-player/).

You can also attach objects to the player by setting them as children of this entity, although the [Attach to Player](/docs/creator/sdk7/3d-essentials/entity-positioning/#attach-an-entity-to-an-avatar) is often the better option for htat.

## The camera entity

The `engine.CameraEntity` entity represents the player's camera.

Fetch the camera's `Transform` component to get the camera's position and rotation. This entity's Transform is also read-only. To modify the camera angle or position, use a [Virtual camera](/docs/creator/sdk7/3d-essentials/camera/#using-virtual-cameras).

You can also fetch the camera's `CameraMode` component to know know if the player is using 1st or 3rd person camera mode, see [camera mode](/docs/creator/sdk7/interactivity/user-data/#check-the-players-camera-mode).
