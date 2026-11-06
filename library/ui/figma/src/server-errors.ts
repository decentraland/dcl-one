import { redactFileKey } from './config.js'

/** The text a tool error or a fatal startup error shows, with the private file key redacted. */
function errorText(error: unknown): string {
  return redactFileKey(error instanceof Error ? error.message : String(error))
}

export { errorText }
