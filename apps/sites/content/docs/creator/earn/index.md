> >-

# How to make money in Decentraland

Decentraland is a free social gaming world for adults, with no corporate parent company. Creators own what they build. There are several ways to earn from what you make: sell Wearables and Emotes in the Shop, enroll a published scene in the Creator Fund, apply for staged funding through Creator Success, or let players support your game directly.

| Option                                                                  | Best for                                                                  | What you receive                                                                    |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [Wearables and Emotes](#sell-wearables-and-emotes)                      | 3D artists and animators                                                  | A share of every sale in the Shop, plus royalties when items are resold             |
| [Creator Fund](#creator-fund-rewards-for-player-engagement)             | Owners of published scenes that already attract players                   | USDC rewards from the Season Pool, based on how players engage with your experience |
| [Creator Success](#creator-success-staged-funding-for-game-developers)  | Independent game developers and small studios with a game ready to grow   | Staged funding, starting with up to USD 4,100 for a first playable version          |
| [Player support](#let-players-support-your-game)                        | Any scene                                                                 | Tips from players, and custom in-scene purchases                                    |

## Sell Wearables and Emotes

Wearables are items of clothing and accessories that avatars wear. Emotes are animations that avatars perform. Both are sold in the Decentraland [Shop](https://decentraland.org/shop), and 3D artists and animators earn from every sale. This is the simplest way to start earning: you don't need a scene, LAND or a NAME.

The path from an idea to a sale looks like this:

1. **Model or animate the item.** Build it in Blender or your preferred 3D tool, following the avatar system and the size limits. See [Creating Wearables](/docs/creator/wearables-and-emotes/wearables/creating-wearables/) and [Creating Emotes](/docs/creator/wearables-and-emotes/emotes/creating-emotes/). The [Wearable Tutorial Series](/docs/creator/tutorials/creating-a-wearable-series/) and the [Emote Videos](/docs/creator/tutorials/video-emotes/) walk through the whole process on video.
2. **Create a collection and upload your items.** Collections group your items and set their rarity, which is how many copies can ever be sold. See [Creating a Collection](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/), [Uploading Wearables](/docs/creator/wearables-and-emotes/manage-collections/uploading-wearables/) and [Uploading Emotes](/docs/creator/wearables-and-emotes/manage-collections/uploading-emotes/).
3. **Publish the collection.** Publishing has a fee per item, currently 25 USD, of which the Decentraland Foundation covers 20 USD for a limited time, so you pay 5 USD per item. The Curation Committee then reviews your items. See [Publishing Collections](/docs/creator/wearables-and-emotes/publishing/publishing-collections/) and the [Curation Committee](/docs/creator/wearables-and-emotes/publishing/curation-committee/).
4. **Put your items on sale.** Once approved, set a price for each item and it goes on sale in the Shop. Each sale carries a 2.5% fee that goes to the Decentraland DAO, so you keep 97.5%. When a player resells your item in the [Marketplace](https://decentraland.org/marketplace), you receive royalties. See [Selling Items](/docs/creator/wearables-and-emotes/publishing/publishing-collections/#selling-items).

If you also write code, a Wearable can carry gameplay with it: a [smart wearable](/docs/creator/sdk7/projects/smart-wearables/) runs a scene whenever a player puts it on, so a jetpack can fly or a fishing rod can fish anywhere in Genesis City.

## Creator Fund: rewards for player engagement

The **Creator Fund** is a rewards program for creators whose published experiences attract and keep real players. Each Cycle, a fixed allocation from the Season Pool is shared among the enrolled experiences in proportion to how players engage with them. Rewards are a proportional share, not a guaranteed amount. There is nothing to apply for and no pitch to write: you publish a scene, join the program, and the activity of real players determines your scene's share.

- **The Season Pool:** the Beta Season runs from November 15 to December 31, 2026, with a Season Pool of USD 30,000, paid in USDC, split over three Cycles. Each month has two reward Cycles.
- **Who can join:** you must be 18 or older and own the LAND, NAME or World where your scene is deployed. Opt in from the **Earn** tab in [Creator Hub](https://decentraland.org/download/creator-hub) by signing the Program Terms with that wallet. Opt-in opens on November 11, 2026. One opt-in covers your eligible experiences, and scenes you publish later join the next time you open the **Earn** tab.
- **What counts:** scenes in Genesis City and Worlds that follow the Decentraland Terms of Use and Content Policy, with no gambling or other 18+ restricted content. Only real players using the official Decentraland desktop and mobile apps count. Rewards follow the new Decentraland players you bring in, active playtime, and players who come back on the days after their first visit.
- **Payouts:** in USDC, within 14 days after each Cycle ends, to the Payout Wallet you set in Creator Hub. Payout Wallet setup opens in Creator Hub on November 25, 2026. If you haven't set one within 7 days after a Cycle ends, you won't receive the Reward for that Cycle. Rewards under USD 5 per Cycle, across all your experiences, are not paid out.
- **Your results:** Creator Hub shows your scenes' metrics during the Cycle. These figures are for information only: final Rewards are calculated after review and bot filtering. After each Cycle, we may publish leaderboards of the top experiences and their Rewards.

Read the full eligibility rules, the Cycle dates and the payout process in [Creator Fund: Program Rules](/docs/creator/earn/program-rules/).

## Creator Success: staged funding for game developers

**Creator Success** supports independent game developers and small studios through a structured development process, with staged funding and feedback from real players.

Funding starts with up to USD 4,100 to develop a first playable game, with further funding opportunities for projects that show strong player engagement and retention.

Learn about the program and its development stages in [Creator Success: Get Paid To Make Games](/docs/creator/earn/studio-onboarding/).

The two programs are separate, and you can take part in both. A game you build through Creator Success is published to a World or LAND you own, so once it is live it can also be enrolled in the Creator Fund if it meets the [eligibility rules](/docs/creator/earn/program-rules/#who-can-take-part).

## Let players support your game

Players can already send you tips in MANA from inside your scene, and you keep 100% of every tip. A player needs MANA in their wallet to send one, so tips come mostly from players who already hold some. Add your wallet address in your scene settings to turn tipping on. See [Tipping](/docs/creator/scene-editor/configure/scene-settings/#tipping).

If you write code, you can also use the SDK to build your own blockchain transactions into your experience, for example entry fees, paid areas and unlocks, content for holders of a certain token, or an in-world shop for your Wearables and Emotes. See [Scene Blockchain Operations](/docs/creator/sdk7/blockchain/scene-blockchain-operations/) to get started.

More in-game purchase features for creators are planned for Q1 2027, to make it easier to earn directly from your games.


> [!WARNING]
> **📔 Note**: Most players don't have a wallet with funds ready to spend. Many play as guests, and players who do have an account often hold no tokens, so a payment prompt is a dead end for them. Design payments as optional extras, never as the only way to play, and check `isGuest` from [player data](/docs/creator/sdk7/interactivity/user-data/#get-player-data) before offering them.


Charging players doesn't affect your scene's place in the Creator Fund, but the Fund's [eligibility rules](/docs/creator/earn/program-rules/#what-counts) still apply: experiences with gambling or other 18+ restricted content don't count.

If you own LAND, you can also [rent it](/docs/player/marketplace/rentals/) to other creators in the Marketplace. If what you want is to hand out items rather than sell them, the [Rewards](/docs/creator/rewards/overview/) service lets a scene give Wearables and Emotes to players, for example as prizes in a game.

## Built in the open

Decentraland is built on open-source software, with no corporate parent company. You can read the code, report issues and contribute on [GitHub](https://github.com/decentraland). Platform fees flow back to the DAO rather than to a company, and the goal behind all of it is for the software of the internet to be collectively owned, not privately profited from. Your creations and your right to profit from them are part of that.
