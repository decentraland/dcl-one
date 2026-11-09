> Social fortune-telling with explicit conversation-state transitions and a pivot-based

# The Fortune Teller

![The Fortune Teller preview](/docs/creator/tutorials/examples/published-games/images/Decentraland_n9oGHbJPnR.webp)

A social roleplay experience where players can receive fortunes or tell fortunes for friends.

- [Play on Decentraland](https://decentraland.org/jump?realm=fortunes.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · carlosmu

[View upstream source](https://github.com/carlosmu/circus_fortune_teller)

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

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Represent a reading as explicit state transitions

The fortune state machine lists allowed steps from category and deck selection through the reveal. `tryTransition` checks the next step, updates the session, and notifies state-entry listeners.

**Project context:** Read it with the session types and action handlers to see how a multi-step interaction is organized.

**Source references:**

- [fortune state machine](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/fortuneFsm/machine.ts#L6-L60)

### Build a camera rig around a scene object

`ensureHostPivotRig` and `startHostCinematicCamera` parent a virtual camera to a pivot under the table, choose starting and ending angles from the player's side, and activate the camera.

**Project context:** The rig depends on the table entity, configuration values, and update system in this module; those pieces explain how to frame an interaction in its scene context.

**Source references:**

- [`ensureHostPivotRig` and `startHostCinematicCamera`](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/cinematicCamera.ts#L141-L197)

<!-- example-highlights:end -->
