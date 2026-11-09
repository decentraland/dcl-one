> A cooperative garden with persistent plant state, server-clock offset estimation, and

# Bloom Garden

![Bloom Garden preview](/docs/creator/tutorials/examples/published-games/images/bloomGarden-preview.webp)

A cooperative garden experience where players work together to maintain a living garden and awaken the Big Bloom.

Its Multiplayer Server uses built-in Storage for plant state, a weekly leaderboard, and per-player watering limits.

- [Play on Decentraland](https://decentraland.org/jump?realm=bloomgarden.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · TheLivingGarden

[View upstream source](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main)

**License:** [MIT](https://github.com/TheLivingGarden/TheLivingGarden/blob/main/LICENSE). Check upstream terms before reuse; code and assets may have different terms.


> [!WARNING]
> **Community-maintained code — review before use**
>
> This community project is maintained outside the Decentraland Foundation and DAO GitHub organizations. Its source code and live deployment may change independently.
>
> Inclusion in this catalog does not imply a security audit or guarantee of safety. Review code, dependencies, and installation scripts before running or reusing it.
>
> [Report a catalog concern](https://github.com/decentraland/docs/issues/new). For suspected vulnerabilities, follow the upstream project's security policy. Do not include exploit details or credentials in public issues.

<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Estimate the offset between server and client clocks

The clock helper keeps five offset samples, rejects large deviations, and converts server timestamps into local time. The watering client feeds it timestamps received through the scene's message definitions.

**Project context:** This illustrates a lightweight timing estimate; network transit time is included in the samples.

**Source references:**

- [The clock helper](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/shared/clockSync.ts#L3-L55)
- [The watering client](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/wateringSystem.ts#L1144-L1146)

### Let temporary effects finish naturally

The petal setup and controls create a reusable pool from the scene's named Petal model, then start or settle the effect. The update phases scale petals in, let them fall and rest, and shrink them before recycling. Study this when an effect should stop spawning while existing objects complete their animation.

**Project context:** Placement depends on BLOOM_CENTER and the module's timing constants.

**Source references:**

- [The petal setup and controls](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L119-L173)
- [The update phases](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L207-L310)

<!-- example-highlights:end -->
