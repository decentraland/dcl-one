type GallerySummary = {
  entry: string
  hosted: boolean
  runtimePages: number
  examples: number
  foundations: number
  hudPages: number
  generatedPages: number
  generated: string
  requests: number
}

export declare function checkGallery(options: {
  dist: string
  prefix?: string
  hosted?: boolean
  browser?: string
}): Promise<GallerySummary>
/**
 * Waits until the pictures and web fonts of the page's first iframe settle, then rejects naming every layout problem
 * in it: a page wider than its viewport, a picture that does not load or paints outside its box, in-flow text over
 * other text.
 */
export declare function checkLayout(
  session: Awaited<
    ReturnType<typeof import('../../figma/review/scripts/capture-browser.mjs').openSession>
  >,
  page: string
): Promise<void>
export declare function main(argv: string[]): Promise<number>
