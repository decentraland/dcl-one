> Multiplayer keep-away with timed rounds, persistent leaderboards, pooled projectiles,

# Flag Tag

![Flag Tag preview](/docs/creator/tutorials/examples/published-games/images/flagTag-preview.webp)

A 24/7 Multiplayer Server keep-away game. Players compete to hold a flag during five-minute rounds, using boomerangs and banana traps while persistent daily, monthly, and all-time leaderboards track results.

- [Play on Decentraland](https://decentraland.org/jump?realm=flagtag.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · iillee

[View upstream source](https://github.com/iillee/flagtag)

**License:** Not identified in this catalog. Consult the upstream repository and maintainer for applicable terms before reuse.


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
- Storage for player progression, unlocks
- Avatar masking for custom animations

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Reuse projectile visuals by model variant

The projectile pool pre-creates ten hidden entities for each boomerang color. Acquire and release helpers find an available entity and return it to its hidden position after stopping its sound.

**Project context:** The flight code must position acquired entities and return them when finished; the pool relies on the project's boomerang assets and sound helper.

**Source references:**

- [The projectile pool](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L11-L37)
- [Acquire and release helpers](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L48-L66)

### Limit active lights around the player

proximityLightSystem checks periodically, selects the nearest lights with squared-distance comparisons and reusable arrays, and changes only lights whose active state differs. The scene caps the active set at eight and gates it through its day/night helper.

**Project context:** Read the setup and constants in the same file to understand the world-position conversion and device-specific intensity settings.

**Source references:**

- [proximityLightSystem](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/proximityLights.ts#L109-L168)

<!-- example-highlights:end -->
