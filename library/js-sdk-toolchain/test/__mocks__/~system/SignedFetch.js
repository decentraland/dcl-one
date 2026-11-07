// No-op stub for the ~system/SignedFetch host module. The smart-items actions runtime and the admin
// toolkit fetch helpers import it at eval time; the specs never invoke it.
const noop = async () => ({})

module.exports = {
  signedFetch: noop,
  getHeaders: noop
}
