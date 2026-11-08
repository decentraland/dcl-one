> A World Cup prediction scene with charged ball kicks, immediate local motion feedback,

# Kick Off

![Kick Off preview](/docs/creator/tutorials/examples/published-games/images/dclKickoff-preview.webp)

A seasonal World Cup prediction game.

- [Play on Decentraland](https://decentraland.org/jump?realm=kickoff.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland DAO

[View source](https://github.com/dcl-regenesislabs/kickoff-2026)

**License:** Not identified in this catalog. Consult the upstream repository and maintainer for applicable terms before reuse.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Connect a charged kick to immediate visual feedback

The charge-and-release loop combines keyboard and mobile input, grows and recolors a charge bar, sends a kick message, and starts local ball motion. `releaseOptimistic` initializes that motion.

**Project context:** Study it with the project's `prodeNet` room and ball message handlers to understand the relationship between input, feedback, and multiplayer messages.

**Source references:**

- [charge-and-release loop](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L491-L536)
- [`releaseOptimistic`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L155-L178)

### Scale UI from a reference canvas

`layoutScale` reads `UiCanvasInformation`, compares both dimensions with a 1920×1080 design space, clamps the result, and applies an additional mobile factor.

**Project context:** It is a compact example of a shared layout helper; the limits and mobile factor are tuned for this project's panels.

**Source references:**

- [`layoutScale`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/schedule/responsive.ts#L1-L21)

<!-- example-highlights:end -->
