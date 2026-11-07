// No-op stubs for the ~system/RestrictedActions host module. The smart-items actions runtime imports
// it at eval time; the specs never invoke these code paths, the stubs only let the module graph load.
const noop = async () => ({})

module.exports = {
  movePlayerTo: noop,
  triggerEmote: noop,
  triggerSceneEmote: noop,
  openExternalUrl: noop,
  teleportTo: noop,
  changeRealm: noop,
  copyToClipboard: noop
}
