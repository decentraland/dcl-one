> Wearable dispensers and airdrop crates that claim rewards from a Rewards campaign.

# Reward claim dispenser

Give players wearables from a Decentraland Rewards campaign. The scene includes a click-to-claim dispenser, a button dispenser, and a lever that airdrops claimable crates.

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland

[View source](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim)

**License:** [Apache-2.0](https://github.com/decentraland/sdk7-goerli-plaza/blob/main/reward-claim/LICENSE). Check upstream terms before reuse; code and assets may have different terms.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Request a reward with a signed fetch

`claimToken` throttles repeated clicks, skips dispensers already claimed successfully in this session, and checks that the player has a wallet. `requestToken` then posts the dispenser key, the current catalyst, and the player's address to the Rewards server using `signedFetch`.

**Project context:** Testing is on by default (`CONFIG_CLAIM_TESTING_ENABLED` in `src/config.ts`), which uses a bundled test campaign. To use your own, replace the placeholder campaign ID and dispenser key in `ClaimConfig` and turn testing off. See the Rewards getting-started guide for creating a campaign and dispenser.

**Source references:**

- [`claimToken` and `requestToken`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L26-L99)
- [`ClaimConfig`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claimConfig.ts#L21-L34)

### Map Rewards responses to player feedback

`processResponse` shows a different dispenser UI for unavailable campaigns, out-of-stock dispensers, rewards already assigned, and successful claims. Uninitiated, disabled, and finished campaigns share one branch, which shows a not-started or ended screen only when the campaign config sets `startDate` or `endDate`, and a generic finished screen otherwise.

**Project context:** The UI states are defined in `dispenserUi.tsx`. Campaign start and end dates are optional fields on the campaign config.

**Source references:**

- [`processResponse`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L101-L197)

### Retry a claim with a captcha

When the Rewards server asks for a captcha, `getCaptcha` fetches a captcha image and shows it in the dispenser UI. Submitting the answer calls `claimToken` again with the captcha ID and value.

**Project context:** Whether a captcha is required is a setting of the dispenser in the Rewards dashboard.

**Source references:**

- [`getCaptcha`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L209-L216)
- [Captcha submit button](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/ui/dispenserUi.tsx#L640-L643)

### Claim from an airdropped crate

`randomCrateSpawn` drops a crate with a parachute at a random point in the stage area. `DropCrate` raycasts down to find the ground and tweens the crate onto it, and clicking the crate claims the reward and makes it vanish.

**Project context:** The crate drop is triggered by the `Activate` action of a Creator Hub lever in `main`.

**Source references:**

- [`randomCrateSpawn` and `SpawnCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L40-L119)
- [`DropCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L142-L189)

<!-- example-highlights:end -->
