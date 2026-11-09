> Platforming with trigger-activated bounce raycasts, player impulses, and collectible

# Shroom Zoom

![Shroom Zoom preview](/docs/creator/tutorials/examples/published-games/images/shroomZoom-preview.webp)

A platforming and time-trial game with double-jumping, gliding, collectibles, and exploration.

- [Play on Decentraland](https://decentraland.org/jump?realm=shroomzoom.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · CanessaDCL

[View upstream source](https://github.com/CanessaDCL/dcl-shroomzoom)

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
- Physics impulses for bouncy mushrooms

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Combine a nearby trigger with a precise contact check

BounceTrigger callbacks enable raycasting while the player is inside a trigger and remove that system on exit. The raycast and bounce logic check downward hits against the configured mushroom entity and apply a player impulse near its surface.

**Project context:** Read it alongside BounceZoom or BounceRegular for the scene's trigger dimensions and impulse settings.

**Source references:**

- [BounceTrigger callbacks](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L75-L101)
- [The raycast and bounce logic](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L103-L164)

### Turn collected items into an animated entourage

Follower target helpers calculate offsets around the player and positions around an orbit. followSpritesSystem interpolates each sprite toward its target, switching to orbiting when the collection is complete and rotating sprites toward their owner.

**Project context:** The scene's collection sets, follower entities, and per-player orbit timestamps supply the state for this visual behavior.

**Source references:**

- [Follower target helpers](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L400-L437)
- [followSpritesSystem](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L678-L706)

<!-- example-highlights:end -->
