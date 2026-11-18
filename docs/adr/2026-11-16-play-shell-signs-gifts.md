# ADR: The `/play` web shell signs gifts under a per-gift guard

- Status: Proposed
- Date: 2026-11-16

## Context

Players send a wearable or emote they own to another player as a gift. A gift is an on-chain transfer of one token
that the sender signs. The Explorer's engine can't sign it: the native remote-signing path has never run end to end,
and the web engine's signer signs EIP-191 messages only. The Sites `/play` web shell holds the player's login and
wallet, and already signs Shop purchases and MANA tips under per-operation guards
([tips ADR](2026-11-12-play-shell-signs-tips.md)).

Unlike a tip, the app must name what is sent: the picker lists the player's own items, and the player picks one and a
recipient there.

## Decision

Gift runs in the `/play` web shell, the way Donate does.

- **The HUD names a token and a recipient, nothing else.** The shared HUD reads Gift from `@hud-host`'s `gifting`
  (`library/explorer-core/hostContract.ts`). It is null on the Explorer's own HUD, so Gift shows nowhere there. `/play`
  implements it over `apps/ui3/src/play/lib/giftProtocol.ts`: `gift.capabilities`, `gift.pending` and `gift.open`, whose
  request carries the recipient's address, the token's urn and id and the token's `transferredAt`.
  Unknown fields are refused.
- **The player confirms in the shell.** The picker's SEND GIFT only opens the shell's confirm step: the item and the
  recipient's name and address as the shell resolved them, with SEND GIFT and CANCEL. Nothing is signed before that
  press, which the app frame can't make: an in-app (managed) wallet signs with no prompt of its own, and a browser
  wallet shows the call as opaque bytes, so without it a script in the app frame could move the player's items. Before
  showing it, the shell checks everything itself:
  - the token urn names a `collections-v2` item on the build's chain;
  - the marketplace index knows that contract's token on that chain;
  - `ownerOf` on chain is the login's account at that moment;
  - the recipient is neither the sender nor the zero address.
  It resolves the recipient's profile and the item from those reads, never from the app.
- **A per-gift guard.** The wallet is wrapped by `shell/gift/guard.ts`. It forwards one request: the collection's
  EIP-712 meta-transaction whose call is `transferFrom(sender, recipient, tokenId)`, for the login's account, on the
  build's chain. It refuses anything else, a transaction sent from the wallet included, re-checks the account and the
  login before the signature, and drops a signature that comes back after the dialog was closed. Donate's guard and
  this one share `shell/metaTxGuard.ts`.
- **Gasless only.** The transfer is relayed as a meta-transaction. Web3 and managed (in-app) wallets both sign typed
  data, so both can gift; a guest can't.
- **Sent when the relay takes it.** The shell shows "Gift Sent" once the relay answers with the transaction's hash,
  keeps the gift as the account's pending gift, and posts `bevy-shell:gift` so the HUD leaves the token out of the
  picker.
- **Never twice, by construction.** A token can move only once, so a second send of the same token can't land. An
  outcome the shell can't tell (the relay's answer lost) is settled by the token's owner, watched for three minutes:
  a token that left the account was sent. A gift the relay refused, or one still the sender's after the watch, is
  shown as failed; TRY AGAIN signs the same meta-transaction nonce while it is unused.
- **Pending gifts are the shell's.** The app frame has no storage of its own, so the shell keeps each account's pending
  gifts in the browser's storage for a week, with the token's `transferredAt` when it was sent. The picker hides a
  pending token only while the owned catalog still lists it with that `transferredAt`.
- **One gift at a time.** A Web Lock per account is held from the send until it settles. The Shop's dialog, the Donate
  panel and the gift's transfer share the one dialog over the app.
- **Switched by the Explorer's flags file.** `explorer-alfa-gifting` turns Gift on, read from the flags service the
  Shop reads.
- **The recipient is told by the catalyst.** It indexes the transfer and sends `transfer_received` with the token's
  metadata URI; the HUD's notification opens the gift-received popup. Nothing is forwarded.

## Consequences

- Gift exists on `/play` only. The desktop Explorer still has no signing path for transfers; it shows the
  gift-received popup, which signs nothing.
- Ethereum (`collections-v1`) items are listed in the picker and refused on SEND: the relay serves Polygon only.
- Gifting takes one more press than the picker alone: the confirm step in the shell.
- A gift the relay accepted but that is never mined stays pending for a week, then shows in the picker again.
