> Spawn a tree of entities and components at runtime from a composite file

# Composites

A _composite_ is a file that describes a tree of [entities and components](/docs/creator/sdk7/architecture/entities-components/). Instead of writing code to create each entity one by one, you can store that structure in a composite file and spawn the whole thing with a single call.

You might already work with composites without realizing it:

- Every scene has a `main.composite` file. It holds everything you added and configured visually in the [Scene Editor](/docs/creator/scene-editor/get-started/about-editor/). This file is loaded automatically when your scene starts.
- [Custom Items](/docs/creator/scene-editor/interactivity/custom-items/) are saved as `composite.json` files. Each Custom Item is a composite that you can reuse across scenes.

This page covers how to spawn a composite from code at runtime, for example to spawn a Custom Item dynamically as part of your scene's logic.

## Spawn a composite

Spawning a composite takes two steps: **load** the composite file, then **spawn** it. Loading is asynchronous and spawning is synchronous, so the pattern is always load-then-spawn:

```ts
import { engine, Composite, getCompositeProvider } from "@dcl/sdk/ecs";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  // 1. Load the composite from its file
  const resource = await provider.loadComposite(src);

  // 2. Spawn it: creates all its entities and components
  const barrel = Composite.instance(engine, resource, provider);
  return barrel;
}
```

`provider.loadComposite(src)` reads the composite file and loads it into memory. In a normal scene built with `@dcl/sdk`, a composite provider is already set up for you, you reach it with the `getCompositeProvider()` function from `@dcl/sdk/ecs`.

`Composite.instance()` then creates all the entities described in the composite, with all their components, and returns the **root entity** of the spawned tree. Use this returned entity to read or change components later, for example to reposition or remove the spawned item.


> [!NOTE]
> **💡 Tip**: To do the same thing without writing code, use the **Spawn Entity** action in the Scene Editor. See [About spawning entities](/docs/creator/scene-editor/interactivity/smart-items-advanced/#about-spawning-entities).



> [!WARNING]
> **📔 Note**: You must load a composite before you spawn it. `Composite.instance()` is synchronous and can only spawn a composite that's already in memory. Only `main.composite` is bundled with your scene and available from the start; any other composite file must be loaded with `loadComposite()` first. To check synchronously whether a composite is already available, use `provider.getCompositeOrNull(src)`.
> 
> `loadComposite()` is idempotent: it keys each composite by its `src` string, so calling it again with the same path doesn't reload the file, it returns the already-loaded composite. You can safely call it before every spawn without worrying about loading the same file twice.


### Spawn onto an existing entity

By default `Composite.instance()` creates a new entity to hold the spawned tree. Pass a `rootEntity` in the options to spawn onto an entity you already have:

```ts
// Spawn the composite directly at the scene root, with no wrapper entity
const barrel = Composite.instance(engine, resource, provider, {
  rootEntity: engine.RootEntity,
})
```

## Position a spawned composite

To place the spawned composite at a specific position, rotation, or scale, set a `Transform` component on the root entity returned by `Composite.instance()`.

```ts
import { engine, Composite, getCompositeProvider, Transform } from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  const resource = await provider.loadComposite(src);

  // Spawn the composite at a specific position
  const barrel = Composite.instance(engine, resource, provider);
  Transform.createOrReplace(barrel, {
    position: Vector3.create(8, 0, 8),
  });
}
```


> [!WARNING]
> **📔 Note**: `Transform.createOrReplace()` replaces the root entity's existing Transform component. Only the root entity is affected, child entities keep their positions relative to the root.


## Current limitation: nested composites

Spawning works for self-contained composites and Custom Items. A composite can reference another composite from one of its entities, but `loadComposite()` does **not** recurse into those nested references: it only loads the file you pass it.

When spawning, `Composite.instance()` recurses into nested references that are already in memory. If a nested composite isn't loaded (anything other than `main.composite`, the only composite bundled with your scene), that branch logs a warning and is skipped. To spawn a composite with nested references, call `loadComposite()` for each referenced file first — or simpler, keep the composites you spawn self-contained.

## Related pages

- [Entities & Components](/docs/creator/sdk7/architecture/entities-components/) — the building blocks a composite describes.
- [Custom Items](/docs/creator/scene-editor/interactivity/custom-items/) — reusable items stored as composites.
- [Smart Items - Advanced](/docs/creator/scene-editor/interactivity/smart-items-advanced/#about-spawning-entities) — spawn composites with no code, using the Spawn Entity action.
- [Scene Files](/docs/creator/sdk7/projects/scene-files/) — where `main.composite` lives in your scene project.
</content>
