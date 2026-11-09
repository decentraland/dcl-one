import { stripFileKeyText } from './capture/strip-file-key.js'
import { redactFileKey } from './config.js'

/** The text a tool error or a fatal startup error shows, with Figma file URLs and the configured key redacted. */
function errorText(error: unknown): string {
  return redactFileKey(stripFileKeyText(error instanceof Error ? error.message : String(error)))
}

export { errorText }
