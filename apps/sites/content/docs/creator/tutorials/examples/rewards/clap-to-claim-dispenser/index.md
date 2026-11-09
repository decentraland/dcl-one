> A wearable dispenser that players claim by doing the clap emote in a trigger area.

# Clap-to-claim dispenser

Let players claim a wearable by doing the clap emote in front of a dispenser, a mechanic used at Metaverse Fashion Week. The README walks through pointing the dispenser at your own Rewards campaign.

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland

[View source](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser)

**License:** Not identified in this catalog. Consult the upstream repository and maintainer for applicable terms before reuse.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Arm a dispenser while the player stands in front of it

Each dispenser stores its campaign in a `DispenserComponent`. A sphere `TriggerArea` in front of it sets `isActive` and shows a "clap to claim" toast on enter, and clears both on exit.

**Project context:** The campaign ID and dispenser key come from `ClaimConfig` in `src/modules/claiming/claimConfig.ts`.

**Source references:**

- [`createDispenserWithWearable`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/dispensers.ts#L28-L89)

### Claim when the player plays the clap emote

`initClapToClaim` watches `AvatarEmoteCommand` on the player entity. When the emote is `clap`, it waits for the animation and claims from every dispenser that is currently active.

**Project context:** The claim request and response handling follow the same Rewards flow as the reward claim dispenser example.

**Source references:**

- [`initClapToClaim`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/clapToClaim.ts#L6-L33)

<!-- example-highlights:end -->
