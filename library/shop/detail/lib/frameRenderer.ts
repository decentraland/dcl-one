// Which renderer draws a Shop avatar in its preview iframe: the Bevy avatar preview where its flag allows it, then
// wearable-preview's Unity where that flag does, then Babylon.


export type FrameRenderer = "bevy" | "unity" | "babylon";

/**
 * Bevy where `shop-bevy-avatar-preview` is on, then Unity where `unity-wearable-preview` is, both on a device
 * pickRenderer allows the full renderer; Babylon otherwise. A renderer that failed here is not asked for again. Null
 * while a full device waits for the flags: a lighter one draws with Babylon whatever they say, so it does not wait.
 */
export function frameRenderer(
  full: boolean,
  flags: { bevy: boolean | null; unity: boolean | null },
  failed: { bevy: boolean; unity: boolean },
): FrameRenderer | null {
  if (!full) return "babylon";
  if (flags.bevy === null || flags.unity === null) return null;
  if (flags.bevy && !failed.bevy) return "bevy";
  if (flags.unity && !failed.unity) return "unity";
  return "babylon";
}

/** The options that ask the iframe for this renderer. Unity's mode is Bevy's too; Babylon has none. */
export function rendererOptions(renderer: FrameRenderer, mode: "marketplace" | "builder") {
  return { bevy: renderer === "bevy" || undefined, unity: renderer === "unity" || undefined, unityMode: renderer === "babylon" ? undefined : mode };
}
