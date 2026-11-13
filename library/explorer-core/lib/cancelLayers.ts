// Explicit cancel layering for leaf UI (the gallery lightbox, an open dropdown): a layer
// registers while it is open and the session's Cancel dispatch asks the TOPMOST layer first,
// stopping there — one layer per press, exactly like the popup/panel steps below it.
//
// This replaces DOM-side interception (catch the cancel key, stopPropagation so the engine
// never resolves it): the key now flows to the engine like any other input and the layering
// is enforced by dispatch order instead of event suppression — which is what lets a GAMEPAD
// Cancel (never a DOM event) peel these layers too. Registration order is the stack order,
// so nested layers must mount innermost-last (they do: a layer only exists while open).
//
// Layers and popups are merged by recency (see useEngineSession): both take their place from one
// counter, and Cancel closes whichever opened last. A layer under a popup (the lightbox under a
// profile card) waits for the popup; a layer inside a popup (the passport's options menu) closes
// before it.

let order = 0
/** The next place in the shared layer/popup order; popups take theirs when they open. */
export function nextLayerOrder(): number {
  return ++order
}

const layers: { onCancel: () => void; order: number }[] = []

/** Register while open/mounted; returns the unregister. The handler closes the layer. */
export function registerCancelLayer(onCancel: () => void): () => void {
  const layer = { onCancel, order: nextLayerOrder() }
  layers.push(layer)
  return () => {
    const i = layers.lastIndexOf(layer)
    if (i >= 0) layers.splice(i, 1)
  }
}

/** Close the topmost layer if it opened after `openedAfter` (the top popup's order). Returns false
 *  if there is none, so dispatch falls through to the popup, then the panels. */
export function dispatchCancelLayer(openedAfter = 0): boolean {
  const top = layers[layers.length - 1]
  if (top == null || top.order <= openedAfter) return false
  top.onCancel()
  return true
}
