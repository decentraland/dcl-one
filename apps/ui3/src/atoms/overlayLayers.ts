export type OverlayLayer = {
  /** True when the node is inside the layer: its surface, or the control that opened it. */
  contains: (target: Node) => boolean;
  onEscape?: () => void;
  onOutside?: (event: Event) => void;
  /** A modal layer hides the layers under it from outside presses. */
  modal?: boolean;
};

const layers: OverlayLayer[] = [];

// Escape goes to the top layer only, and stops there: a select inside a dialog closes alone, and no other
// Escape handler on the page (an older modal, another component kit) closes its own surface for the same key.
function onKeyDown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  const top = layers[layers.length - 1];
  if (!top?.onEscape) return;
  event.stopPropagation();
  event.preventDefault();
  top.onEscape();
}

// A press inside a layer is also inside every layer under it, because the upper one opened from the lower one.
function onPointerDown(event: Event) {
  const target = event.target;
  if (!(target instanceof Node)) return;
  for (const layer of [...layers].reverse()) {
    if (layer.contains(target) || layer.modal) return;
    layer.onOutside?.(event);
  }
}

/** True when no other layer opened after this one. */
export function isTopLayer(layer: OverlayLayer): boolean {
  return layers[layers.length - 1] === layer;
}

/** Adds a layer on top of the open ones. Returns the function that removes it. */
export function pushLayer(layer: OverlayLayer): () => void {
  if (layers.length === 0) {
    document.addEventListener("keydown", onKeyDown, true);
    document.addEventListener("pointerdown", onPointerDown, true);
  }
  layers.push(layer);
  return () => {
    const index = layers.indexOf(layer);
    if (index >= 0) layers.splice(index, 1);
    if (layers.length === 0) {
      document.removeEventListener("keydown", onKeyDown, true);
      document.removeEventListener("pointerdown", onPointerDown, true);
    }
  };
}
