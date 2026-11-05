import { emptyRetirements } from './guard/retirements.js'
import type { FigmaSource } from './sources/types.js'

// Single-file by construction (FIGMA_FILE_KEY from the environment): retirements carry no file key, so
// there is no file-identity check to make.
async function sourceRetirements(source: Pick<FigmaSource, 'getRetirements'>) {
  return (await source.getRetirements?.()) ?? emptyRetirements()
}

export { sourceRetirements }
