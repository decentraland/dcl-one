import { stripFileKeyText } from './capture/strip-file-key.js'

/** The text a tool error or a fatal startup error shows, with any Figma file URL replaced by the placeholder. */
function errorText(error: unknown): string {
  return stripFileKeyText(error instanceof Error ? error.message : String(error))
}

export { errorText }
