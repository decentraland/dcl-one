> Reusable NPC behaviours and dialogue UI for SDK7 scenes.

# NPC Toolkit

Use reusable NPC behaviours and dialogue UI for SDK7 scenes.

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland

[View source](https://github.com/decentraland-scenes/dcl-npc-toolkit)

**License:** [Apache-2.0](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/main/LICENSE). Check upstream terms before reuse; code and assets may have different terms.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Separate revealing dialogue from advancing it

`handleDialogClick`, `rushText`, and `confirmText` first reveal the remaining characters if text is still typing. Subsequent input advances the script or selects a numeric dialogue branch, with callbacks for game actions.

**Project context:** Read this alongside the toolkit's `Dialog` data and `npcDialogComponent` state to understand how UI input drives a conversation.

**Source references:**

- [`handleDialogClick`, `rushText`, and `confirmText`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/dialog.ts#L489-L549)

### Connect proximity and pointer input to an NPC interaction

`addClickReactions` and `addTriggerArea` configure pointer callbacks and a child sphere trigger, with settings for click-only activation and continuing dialogue when the player walks away. `activate` and `endInteraction` coordinate the active NPC, cooldown, dialogue, and optional player-facing behavior.

**Project context:** These functions rely on the toolkit's callback maps and tracking system.

**Source references:**

- [`addClickReactions` and `addTriggerArea`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L410-L531)
- [`activate` and `endInteraction`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L769-L823)

<!-- example-highlights:end -->
