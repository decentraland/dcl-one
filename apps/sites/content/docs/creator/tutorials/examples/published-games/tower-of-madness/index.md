> Timed platforming through changing tower sections, with asset preloading and a

# Tower of Madness

![Tower of Madness preview](/docs/creator/tutorials/examples/published-games/images/4c85255e-c1db-48f0-8e0f-35dec36f3ab0-1792498908.webp)

A timed platforming game where players race to reach the top.

- [Play on Decentraland](https://decentraland.org/jump?realm=towerofmadness.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland DAO

[View source](https://github.com/dcl-regenesislabs/towerofmadness)

**License:** Not identified in this catalog. Consult the upstream repository and maintainer for applicable terms before reuse.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Preload the pieces of a changing level

`setupChunkPreload` deduplicates the chunk asset paths and creates temporary GLTF entities. A system watches their loading states, removes entities after loading completes or fails, and unregisters itself when none remain.

**Project context:** This illustrates managing a preload lifecycle; the asset list comes from `shared/chunks` and is specific to the tower.

**Source references:**

- [`setupChunkPreload`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/chunkPreload.ts#L22-L61)

### Build a skippable camera tour around changing geometry

`playCinematic` adjusts camera keyframes around the current start and end chunks, then selects a virtual camera. The update system interpolates position and look-at targets and supports holding a button to skip.

**Project context:** Follow the chunk lookup helpers and `skipCinematic` to see how the tour finds its anchors and returns control to the player camera.

**Source references:**

- [`playCinematic`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L110-L209)
- [update system](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L286-L334)

<!-- example-highlights:end -->
