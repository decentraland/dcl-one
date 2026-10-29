const TAG = '[editor-scene]'
let debugOn = false

export function setSceneDebug(on: boolean): void {
  debugOn = on
}

export const log = {
  debug: (...args: unknown[]): void => {
    if (debugOn) console.log(TAG, ...args)
  },
  warn: (...args: unknown[]): void => {
    console.log(TAG, 'WARN', ...args)
  },
  error: (...args: unknown[]): void => {
    console.error(TAG, ...args)
  }
}
