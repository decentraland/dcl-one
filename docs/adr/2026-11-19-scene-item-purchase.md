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
  {contract}:{id}`), so nothing else from the scene reaches the HUD. One request is in flight at
  a time across all scenes; a second is `WAS_ALREADY_OPEN`.
- **The HUD owns availability.** The engine sends the request on a `SystemApi` stream and holds
  the scene's call until the HUD answers `accepted` or `unavailable`, for at most two seconds.
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
  gone. `/item_purchase_cancel` frees a request the HUD never finishes.
- A urn that names no collection item is `OPENED` then `Failed` where purchases run, with nothing
  shown, and `REJECTED_FEATURE_DISABLED` elsewhere.
- An item purchase shares the `OpenExplorerUi` permission with the menu pages; the purchase
  dialog is the confirmation, so there is no second permission type.
