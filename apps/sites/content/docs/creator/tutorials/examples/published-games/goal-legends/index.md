> Multiplayer penalty shootouts with server-run matches, persistent leaderboards,

# Goal Legends

![Goal Legends preview](/docs/creator/tutorials/examples/published-games/images/goalLegends-preview.webp)

A multiplayer penalty shootout game. Duel another player or the GL-Bot: pick left, center or right to score or save. Every round ends with a replay in the style of a TV broadcast, with the crowd reacting live.

- [Play on Decentraland](https://decentraland.org/jump?realm=goallegends.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · carlosmu

[View upstream source](https://github.com/carlosmu/goal-legends-arena)

**License:** [Apache-2.0 notice](https://github.com/carlosmu/goal-legends-arena/blob/master/LICENCE). Check upstream terms before reuse; code and assets may have different terms.


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

- **Multiplayer Server** runs the matches
- **Storage** for daily/all-time PvP/PvE leaderboards, and each player’s country flag.
- **Scene** **emotes**
- Animator-synced ball props for kicks, dives, goals and saves.
- **Virtual cameras** for the gameplay views and a winner cinematic everyone sees.
- **AvatarShape** crowd NPCs that react to each goal and save.
- **Particles** for the fireworks
- A portal with lights and **`GltfNodeModifiers`** that takes you to another World.

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Switch cameras with match phases

`initGameplayCamera` selects gameplay, winner, or standard cameras from the client match snapshot. The winner camera orbits a pivot, while spectators can choose a free camera during play.

**Project context:** Follow this alongside `clientSnapshot`, the `GameState` enum, and the named scene camera pivot.

**Source references:**

- [`initGameplayCamera`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/gameplayCamera.ts#L109-L165)

### Coordinate a prop animation with the game state

`tickAnimationManager` starts a ball clip when the phase changes, chooses goal/save animations from the shot direction, and briefly hides the prop while changing clips. This path is specific to the bot kicker; human kickers carry the ball in their emotes.

**Project context:** It depends on the scene's ball model and named animation clips.

**Source references:**

- [`tickAnimationManager`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/animationManager.ts#L78-L114)

<!-- example-highlights:end -->
