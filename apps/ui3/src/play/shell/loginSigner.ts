// The login the shell holds as a request signer (ADR-44 signed fetch), for the shell's own signed requests: the
// in-world Shop's purchases (./shop/purchase) and Donate's tip forward (./donate/donation). A small module of its
// own, so Donate reaches it without loading the Shop's purchase code.
import type { AuthIdentity as SignerIdentity } from '../../data/auth/identity'
import { identitySigner } from '../../shop/api/host'
import type { ShopSigner } from '@shop/api/signer'
import { loginExpired, rootAddress, type StoredLogin } from '@explorer-core/features/auth/sso'

/** The login as a signer, or null without a login, its key, or once it has expired. */
export function loginSigner(login: StoredLogin | null): ShopSigner | null {
  const key = login?.identity.ephemeralIdentity.privateKey
  if (!login || !key || loginExpired(login)) return null
  const { identity } = login
  return identitySigner({
    signer: rootAddress(identity),
    ephemeral: { address: identity.ephemeralIdentity.address, privateKey: key as `0x${string}` },
    expiration: String(identity.expiration),
    // The stored chain is the auth site's JSON; its link types are the signer's own.
    authChain: identity.authChain as SignerIdentity['authChain']
  })
}
