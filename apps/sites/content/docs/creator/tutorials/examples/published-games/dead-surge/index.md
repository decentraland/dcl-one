> Zombie survival with weapon progression, pooled reward text, and local enemy steering

# Dead Surge

![Dead Surge preview](/docs/creator/tutorials/examples/published-games/images/deadSurge-preview.webp)

Zombie survival game with combat and weapon progression.

- [Play on Decentraland](https://decentraland.org/jump?realm=deadsurge.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland DAO

[View source](https://github.com/dcl-regenesislabs/dead-surge)

**License:** Not identified in this catalog. Consult the upstream repository and maintainer for applicable terms before reuse.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Reuse entities for floating reward text

`spawnZcRewardTextAtPosition` and its pool initializer cycle through a fixed set of text entities. The `rewardTextSystem` makes them rise, face the camera, and fade before hiding them for reuse.

**Project context:** Read the scene's `RewardTextComponent` and game-time updates with these functions to understand their lifetime tracking.

**Source references:**

- [`spawnZcRewardTextAtPosition` and its pool initializer](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L592-L640)
- [`rewardTextSystem`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L815-L850)

### Steer enemies around each other and simple obstacles

`getZombieSeparationDirection` combines repulsion from nearby zombies. `resolveZombieMovePosition` tries the desired direction, a fallback, and individual movement axes against circular brick obstacles.

**Project context:** This illustrates local steering; the behavior depends on the scene's movement entries, obstacle positions, and radius constants.

**Source references:**

- [`getZombieSeparationDirection`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L868-L896)
- [`resolveZombieMovePosition`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L898-L939)

<!-- example-highlights:end -->
