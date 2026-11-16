# ADR: The `/play` web shell signs MANA tips under a per-tip guard

- Status: Proposed
- Date: 2026-11-12

## Context

Players tip a scene's creator in MANA (Donate, on the minimap and the map's place panel) and will send a wearable
or emote as a gift. Both are on-chain transfers the player signs.

The Explorer's engine cannot sign them. On native, the remote-signing path has never run end to end. On the web,
the engine's signer signs EIP-191 messages only. The Sites `/play` host is different: its web shell (the
top-level page around the app's frame) holds the player's login and wallet, and already signs in-world Shop
purchases through a provider that lets through only the confirmed purchase (`apps/ui3/src/play/shell/shop`).

## Decision

Donate runs in the `/play` web shell, the same way the Shop's purchases do.

- **The HUD names a place, nothing else.** The shared HUD reads Donate from `@hud-host`'s `donations`
  (`library/explorer-core/hostContract.ts`). It is null on the Explorer's own HUD, so no button shows there.
  `/play` implements it over a protocol of its own, `apps/ui3/src/play/lib/donateProtocol.ts`, and the request
  carries a Genesis City parcel or a World name only. The shell resolves the place's creator itself from the
  places service. The recipient, the amount, the token and the chain never come from the app.
- **The shell draws the panel.** The amount is chosen and confirmed in the shell's own overlay, where the wallet
  is, and the overlay shows the creator the shell resolved.
- **A per-tip guard.** The wallet the shell signs with is wrapped by `shell/donate/guard.ts`. It forwards one
  request: the MANA token's EIP-712 meta-transaction whose call is `transfer(creator, amount)`, for the login's
  account, on the build's chain. It refuses anything else, a transaction sent from the wallet included, before it
  reaches the wallet, and re-checks the account and the login before the signature.
- **Gasless only, never twice.** The tip is relayed as a meta-transaction and nothing else: a transaction from the
  wallet beside it would not use up its nonce, so both could land. The relay's refusal (`ok: false`, which it also
  answers after a broadcast it lost) fails the tip. The account's next send first reads the token's nonce. While it
  is still the failed tip's, the new tip is pinned to that nonce (the guard refuses any other), so at most one of them
  can land; every attempt of that nonce (TRY AGAIN at another amount is one) is remembered. Once the nonce has moved
  past it, nothing is signed: those attempts are watched for their Transfer, and the one mined is shown as sent. Any outcome the shell can't tell apart from a broadcast (a lost answer, a transaction not yet mined) is
  shown as processing and watched, never retried.
- **Confirmed by its own Transfer.** An unclear tip is watched by its hash, when there is one, and by the MANA
  token's `Transfer(tipper, creator, amount)` after the block read just before signing, which also gives the hash the
  creator's notification needs.
- **Failed only while unused.** A relay refusal, or a revert of the known transaction, fails the tip (with TRY
  AGAIN) only while the token's meta-transaction nonce is still the one the tip signed. Once it has moved past it,
  the signature was used, so the tip's Transfer is looked for and the tip is never retried: a retry would sign the
  next nonce and could pay twice. A known hash that reverted is not watched again; its Transfer is.
- **Switched by the Explorer's flags file.** `explorer-alfa-donations` turns Donate on, and
  `explorer-alfa-recommended-donation-amount` names the suggested amounts (`166,333,500` without it), read from the
  flags service the Shop reads (`window.__SHOP_FEATURE_FLAGS__`, else Decentraland's) each time Donate opens.
- **Web3 sign-ins only.** Donate is offered, and `donate.open` is accepted, only for a web3 wallet sign-in, as
  AGENTS.md requires for MANA. The shell checks this on every open, not only in what the HUD reads.
- **One dialog over the app.** The Shop's dialog and the Donate panel each refuse to open while the other's is up.
- **The creator is told.** Once the tip is mined, the shell forwards its hash to the catalyst's
  `/events-notifier/forward-blockchain`, signed by the login, which then sends the creator `tip_received`.

Gifting, the next feature, follows the same decision with its own intent: an NFT `transferFrom` of one token
the player owns.

## Consequences

- Donate exists on `/play` only. The desktop Explorer still has no signing path for transfers.
- The relay cannot tell a web3 sign-in from an in-app one, so restricting Donate to web3 sign-ins is the
  shell's check, not the relay's.
- The recipient is the places service's `creator_address`, which dcl-one derives from the scene's deployer
  (`services/catalyst/explore/src/places/derive.rs`).
- A player whose tip the relay refuses cannot tip from the wallet instead.
- One tab at a time sends or watches an account's tip: a Web Lock per account (as the Shop takes one per purchase)
  is held from SEND until the tip settles. Another tab opening Donate meanwhile says a tip is in progress there and
  goes on once the lock is let go; it can't start a tip of its own. The browser lets a closed tab's lock go.
- A tip whose outcome is unknown is the account's pending tip, kept in the browser's storage (with an id) until it
  settles or its watch ends: after a reload, the page takes the lock again and shows it, not a new form.
- The failed tip a next send checks is kept in the tab's memory for its watch window: after a reload, the next tip
  signs the token's current nonce.
- A tip that is mined only after its 30-minute watch ends is never forwarded, so its creator gets no notification.
- A second tip of the same amount to the same creator, signed in the watch window from another tab, could be taken
  for the first one's Transfer.
