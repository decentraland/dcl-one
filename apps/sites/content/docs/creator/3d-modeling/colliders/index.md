> Learn how to add colliders to 3D models imported to Decentraland.

# Colliders

To enable collisions between a 3D model and users of your scene, you must give the model colliders. Without a collider, players are able to walk through models as if they weren’t there. Colliders are also required to make an object respond to the player’s pointer events or to block them, or to be intersected by raycasts or block them. For performance reasons, colliders are often modeled separately as a simpler invisible geometry that has less vertices than the visible geometry.


> [!NOTE]
> **💡 Tip**: Install the [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). It includes several handy functions to help you edit and export 3D models, wearables, and emotes.


The model can either use an invisible geometry to use as colliders, or (as of SDK7) you can instead assign the visible geometry to behave as a collider. See [**Colliders on 3D models**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) . You can also assign different layer of collisions to the different geometries, to separately control player physics, pointer events, and other custom layers, see [**Colliders on 3D models**](/docs/creator/sdk7/3d-essentials/colliders/#collision-layers) .

Colliders don’t affect how models and entities interact with each other, they can always overlap. Colliders only affect how the model interacts with the player’s avatar.

For an object to be recognized by a Decentraland scene as a collider, all it needs is to be named in a certain way. The object’s name must include the the suffix “\_collider” at the end.

For example, to create a collider for a lamp post you can create a simple box object surrounding its trunk. Players in the scene won’t see this box, but it will block their path.

![](/docs/.gitbook/assets/25-collider.webp)

In this case, we can name the box “LampPos&#x74;_&#x63;ollider” and export both the tree and the box as a single \_\_gltf model. The \*\_\_collider_\* tag alerts the Decentraland world engine that the box object belongs to the collection of colliders, making the collider mesh invisible.

Whenever a player views the lamp post model in your scene, they will see the complex model for your lamp. However, when they walk into it, they will collide with the box, not the tree.

## Player Spatial Constraints

When you design models that are meant for players to walk around in, you need to keep in mind the following reference measurements:

* Players can walk up a step of up to 30 centimeters in height without jumping
* Players can walk up slopes of an angle of up to 45 degrees without jumping
* Players can jump up to 1,8 meters in height
* Avatars measure around 1.9 meters in height, but consider that they might have hats or other wearables on their head.

Whenever a player views the tree model in your scene, they will see the complex model for your tree. However, when they walk into your tree, they will collide with the box, not the tree.

### Add A Collider To a Staircase

Stairs are a very common use-case for collider objects. In order for players to climb stairs, there must be a corresponding \_collider object that the players are able to step on.

We recommend using a ramp object for your stair colliders, this provides a much better experience when walking up or down. When they climb up your stairs, it will appear as a smooth ascent or descent, instead of requiring them to “jump” up each individual step.

Using a ramp object also avoids creating unnecessary geometry, saving room for other more complicated models. Keep in mind that collider geometry is also taken into account when calculating the [**scene limitations**](/docs/creator/sdk7/optimizing/scene-limitations/)

1. Create a new object in the shape of a ramp that resembles the size and proportions of the original stairs.

![](/docs/.gitbook/assets/26-collider-2.webp)

2. Name the ramp object something similar to &#x53;_&#x74;air\_collider_. It must end in \__collider_.
3. Overlay the ramp object to the stairs so that they occupy the same space.

![](/docs/.gitbook/assets/27-collider.webp)

4. Export both objects together as a single _glTF_ model.

Now when players view the stairs in your scene, they’ll see the more elaborate model of the stairs, but when they climb them, they’ll collide with the ramp.

## Optimizing Colliders

* Prefer using **Box**, **Sphere** and **Capsule** collision meshes, because it will be more performant on loading and at runtime.
* Consider approximating complex collision mesh by several Primitive collision meshes (**Box**/**Sphere**/**Capsule**)

![](/docs/.gitbook/assets/28-optimizing-colliders.webp)

_Example: Collider for Ring can be composed as several Capsule or Box colliders_

* Put collision mesh inside the Hierarchy of the object to which it is related. For example Spaceship\_collider can be a child inside Spaceship parent (from Hierarchy point of view)

### Convex vs Concave Colliders

If using arbitrary mesh collider is the only possibility, then make collision mesh Convex (not Concave). It will prevent strange physics situations, like stuttering, stuckings inside the collider or clipping through it. Note, that it is often possible to mimic concave mesh by several convex meshes.

![](/docs/.gitbook/assets/29-convex-collider.webp)

## Collision Naming

**Use meaningful names. Name should give context of where the asset is used or to which part of the object it relates.**

For asset naming we use mix of `PascalCase` and `snake_case`, which we can call `PascalSnake_case`. Basic rules there - new word or word after separator (`_`) starts with **Capital letter.**

Collision Mesh is the mesh which is used as reference for creating a collider in the Explorer

* It follows same principles as Meshes above, but has `_collider` suffix at the end **Examples:** 🟢 **Prefer** names - `Tram_Wheel_LeftFront_collider` 🔴 **Avoid** names - `Primitive.001_Collider`, `collider (1)`, `Cylinder.007_collider`,


> [!WARNING]
> **🔥Optimization Tip🔥**
> 
> If a collider can be approximated by a Box, Sphere or Capsule shape then use additional suffix with one of these 3 possible variations \_box, \_sphere or \_capsule. Using these 3 shapes instead of arbitrary mesh for collision detection will increase your scene performance and loading time 💪🚀
> 
> Examples: 🟢 Teater\_MainWall\_North\_collider\_box, Fountain\_Statue\_collider\_sphere


## Best Practices For Colliders

* **Always use the smallest number of triangles possible when creating colliders.** Avoid making a copy of a complex object to use as a collider. Simple colliders guarantee a good user-experience in and keep your scene within the triangle limitations.
* **Collider objects shouldn’t have any material**, as players of your scene will never see it. Colliders are invisible to players.
* **All collider objects names must end with \_**_**collider**_**.** For example, &#x54;_&#x72;ee\_collider_.
* **If you use a \_plane**\_\*\* as a collider, it will only block in one direction.\*\* If you want colliders to block from both sides, for example for a wall, you need to create two planes with their normals facing in opposite directions.
* When duplicating collider objects, pay attention to their names. Some programs append a \__1_ to the end of the filename to avoid duplicates, for example _tree\_collider\_1_. Objects that are named like this will be interpreted by the Decentraland World Engine as normal objects, not colliders.
* You can avoid adding a collider mesh if you add an invisible `MeshCollider` component to the entity, that approximates the shape of the 3D model.
* You can also avoid adding a collider mesh if you configure the `GLTFContainer` component to use the visible geometry as a collider. See [**Colliders on 3D models**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) .
* If you’re creating a 3D model with many mesh shapes that each need their collider, there are some handy add-ons that can help. For Blender you can use the batch rename option inside Blender \*\*\*\*to automatically add a `_collider` suffix to all mesh names in a group. More about this tool on “**Using Blender to rename assets” section**
