// The device's preview decision, measuring the link first when it is all that is left to decide: a phone, data
// saver or a device below the bar has already settled on `lite`, and that visitor does not pay for the download.
// One probe per page load, shared by every preview on it (as shop@ae3083ac's useMeasuredMbps).

import { measureDownlinkMbps } from "@web-core/connection-speed";

import { pickRenderer, type RendererDecision } from "./pickRenderer";

let link: Promise<number | null> | undefined;

function measuredLink(): Promise<number | null> {
  link ??= measureDownlinkMbps();
  return link;
}

/**
 * Calls `decided` with the device's decision: at once when the device alone settles it, after the probe otherwise.
 * Returns the cleanup that drops a decision arriving after the caller is gone.
 */
export function decidePreviewDevice(decided: (decision: RendererDecision) => void): () => void {
  const first = pickRenderer();
  if (first.reason !== "optimistic-default") {
    decided(first);
    return () => {};
  }
  let live = true;
  void measuredLink().then((mbps) => {
    if (live) decided(pickRenderer(mbps));
  });
  return () => {
    live = false;
  };
}

/** Forgets the page's probe reading (tests). */
export function resetMeasuredLinkForTests(): void {
  link = undefined;
}
