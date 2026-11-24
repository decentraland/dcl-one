export type HudScale = { width: number; height: number; min: number; max: number }

export const HUD_SCALE_SOURCE: string
export function readHudScale(source?: string): HudScale
export function expectedHudScale(scale: HudScale, innerWidth: number, innerHeight: number): string
export function expectedWindowHeight(
  width: number,
  aspectRatio: string,
  minBlockSize: string
): number
