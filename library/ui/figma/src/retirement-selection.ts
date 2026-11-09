import { emptyRetirements } from './guard/retirements.js'
import type { FigmaSource } from './sources/types.js'

// Single-file by construction (the one file figma-design-system captures): retirements carry no file key, so
// there is no file-identity check to make.
async function sourceRetirements(source: Pick<FigmaSource, 'getRetirements'>) {
  return (await source.getRetirements?.()) ?? emptyRetirements()
}

export { sourceRetirements }
