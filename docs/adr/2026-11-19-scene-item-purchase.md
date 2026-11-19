# ADR: a scene's item purchase is confirmed by the web shell

- Status: Accepted
- Date: 2026-11-19

## Context

The protocol lets a scene ask the Explorer to open a purchase confirmation for one item:
`RestrictedActions.openExplorerUi` with `EU_ITEM_PURCHASE`, an item urn and a `request_id`, answered
by `OpenExplorerUiResult` and, once the confirmation closes, by `PBExplorerItemPurchaseResult` on
the scene's root entity. The SDK's `openExplorerUiAndWait` speaks it.

In this Explorer the buyer's wallet is held by the `/play` web shell, the top-level page around
the HUD frame. The in-world Shop already buys through it: the HUD names an item
(`shop.buy`, `library/ui/apps/shop/shopProtocol.ts`), and the shell reads the terms, shows its
purchase dialog, signs and settles. The engine and the HUD outside that shell (desktop, the
`react-web` host, a credentialless embed, a headless server) hold no wallet and cannot buy.

## Decision

A scene's item purchase reuses that path, in three parts with one owner each.

- **The engine owns admission** (`crates/restricted_actions/src/item_purchase.rs`). In Rust, in
  order: the player stands in the calling scene; a player action reached the scene within the
  last second; the scene is not a portable experience; the process is not a headless server and
  a HUD is listening; the `OpenExplorerUi` permission allows it. The scene's urn is parsed there
  to a collection contract and an item id (`urn:decentraland:{matic|amoy}:collections-v2:
  {contract}:{id}`, the id in the one spelling the Shop takes, without leading zeros), so nothing
  else from the scene reaches the HUD. One request is in flight at a time across all scenes; a
  second is `WAS_ALREADY_OPEN`, refused before the permission is consulted so the player is never
  prompted about a request that cannot open. A prompt answered after the player left the scene is
  `REJECTED_NOT_CURRENT_SCENE`.
- **The HUD owns availability.** The engine sends the request on a `SystemApi` stream and holds
  the scene's call until the HUD answers `accepted` or `unavailable`. The session answers on
  receipt. The engine sets no deadline on that answer: the HUD opens the confirmation as it
  accepts, so a request the engine gave up on could still be bought there with no result for the
  scene. An unanswered request ends only when no HUD is listening any more.
  The engine sees the bridge scene's stream, not the HUD page, so the bridge scene keeps the
  request in flight and settles it when a page says `hello` (it started late or reloaded): a
  request no page answered is sent again, and one a previous page accepted is reported
  `Dismissed`, because the page that followed its purchase is gone.
  A host that can buy gives the shared HUD a `purchaseSceneItem` function; the `/play` host does,
  inside the shell with the in-world Shop on. Every other host answers `unavailable`, which the
  scene receives as `REJECTED_FEATURE_DISABLED`.
- **The shell owns the purchase.** The HUD sends the same `shop.buy` request as the Shop's BUY,
  for one unit paid in Credits, and follows the operation. The price, the buyer and what is
  signed are the shell's, as for every in-world purchase.

The scene learns a verdict and nothing else: `Purchased`, `Dismissed` or `Failed`, with its urn and
`request_id`. No price, balance, transaction or reason crosses back.

## Consequences

- No purchase from a scene on desktop, in the `react-web` host or on a headless server. A scene
  reads `REJECTED_FEATURE_DISABLED` there and can offer something else.
- The protocol has no result for a purchase still settling. Such a purchase reports `Dismissed`
  when the shell hands it off, so `Dismissed` is not proof that nothing was bought. A purchase the
  buyer starts again from the dialog is a new operation the scene is not told about.
- A request stays in flight until the HUD reports its end, even when the requesting scene has
  unloaded, because the confirmation is still on screen. Nothing is written to a scene that is
  gone. `/item_purchase_cancel` is a debugging action that frees a request the HUD never
  finishes; it does not close a confirmation the HUD has open, whose later reports are ignored.
- When the shell answers a request with an earlier purchase of the same item that is still
  settling, the scene's request is `Dismissed` at once, so one slow purchase does not hold every
  scene's next request.
- A urn that names no collection item is `OPENED` then `Failed` where purchases run, with nothing
  shown, and `REJECTED_FEATURE_DISABLED` elsewhere.
- The urn's network is checked for its form and not carried further: the shell buys on the chain
  it was built for. A urn of the other network opens the shell's dialog, which finds no such item
  to buy, and the scene reads `Failed` when it closes.
- An item purchase shares the `OpenExplorerUi` permission with the menu pages; the purchase
  dialog is the confirmation, so there is no second permission type. The permission's description
  names the purchase confirmation, and a prompt for it names the parsed item (its id and
  collection), never text the scene wrote.
- A shell that has not answered the HUD within 30 seconds (a slow first load) is `Dismissed`,
  not `Failed`: it may still open the dialog once loaded.
