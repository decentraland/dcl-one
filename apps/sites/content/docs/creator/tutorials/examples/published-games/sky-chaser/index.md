> Multiplayer package-catching rounds with persistent leaderboards, pooled projectiles,

# Sky Chaser

![Sky Chaser preview](/docs/creator/tutorials/examples/published-games/images/skyChaser-preview.webp)

A multiplayer game where players compete to catch escaping packages in each round. The scene includes weekly and all-time leaderboards.

- [Play on Decentraland](https://decentraland.org/jump?realm=skychaser.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · stom66

[View upstream source](https://github.com/stom66/dcl-sky-chaser)

**License:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-sky-chaser/blob/main/LICENSE.md). Check upstream terms before reuse; code and assets may have different terms.


> [!WARNING]
> **Community-maintained code — review before use**
>
> This community project is maintained outside the Decentraland Foundation and DAO GitHub organizations. Its source code and live deployment may change independently.
>
> Inclusion in this catalog does not imply a security audit or guarantee of safety. Review code, dependencies, and installation scripts before running or reusing it.
>
> [Report a catalog concern](https://github.com/decentraland/docs/issues/new). For suspected vulnerabilities, follow the upstream project's security policy. Do not include exploit details or credentials in public issues.

<!-- example-context:end -->

## Key features

- Multiplayer server
- Synced gameplay via synced Mulberry32 RNG
- Storage API for leaderboards
- Player locomotion tweaks and physics forces for booster/trampolines/explosions
- Particle systems (boost trails, explosions, pickups)

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Reuse projectile entities during play

`fireProjectile` and `getIdleProjectile` reuse inactive projectiles, create new ones when the pool is exhausted, and update only active projectiles.

**Project context:** Follow the project's `Projectile` class and event handlers for movement, collision responses, and round-end cleanup.

**Source references:**

- [`fireProjectile` and `getIdleProjectile`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/projectileManager.ts#L76-L124)

### Seed pickup placement from round state

`onGameStart` and `getRandomPosition` use the round start time to initialize the project's random generator, then sample pickup distances and heights.

**Project context:** Read the pickup configuration and component store alongside it: mobile limits and later pickup activity affect the sequence of random calls, so a shared seed alone does not guarantee identical layouts on every client.

**Source references:**

- [`onGameStart` and `getRandomPosition`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/spawners/pickupSpawner.ts#L115-L164)

<!-- example-highlights:end -->
