import { describe, expect, it, vi } from 'vitest'

import type { AuthIdentity } from '../../auth/sso'
import { signFetch, type SignMessage } from './sign'

const IDENTITY: AuthIdentity = {
  ephemeralIdentity: { address: '0xephemeral', publicKey: '0xpub' },
  expiration: '2999-01-01T00:00:00.000Z',
  authChain: [{ type: 'SIGNER', payload: '0xowner', signature: '' }]
}

describe('signFetch', () => {
  describe('when the request carries mixed-case metadata', () => {
    it('should sign the crypto-middleware 6.x payload with the metadata sent verbatim', async () => {
      const sign = vi.fn<SignMessage>(async () => '0xsigned')
      const metadata = { signer: 'decentraland-kernel-scene', sceneId: 'Bafk', realm: { hostname: 'Boedo.dcl.eth' } }

      const headers = await signFetch(IDENTITY, 'DELETE', 'https://storage.example/Players/0xAB/values?Key=a', sign, metadata)

      const sent = headers['x-identity-metadata']
      expect(sent).toBe(JSON.stringify(metadata))
      const payload = `delete:/players/0xab/values:${headers['x-identity-timestamp']}:${sent}`
      expect(sign).toHaveBeenCalledWith('0xephemeral', payload)
      expect(JSON.parse(headers['x-identity-auth-chain-1'])).toEqual({ type: 'ECDSA_SIGNED_ENTITY', payload, signature: '0xsigned' })
    })
  })

  describe('when the request goes to the project service', () => {
    it('should sign the path with its query, as that service verifies it', async () => {
      const sign = vi.fn<SignMessage>(async () => '0xsigned')

      const headers = await signFetch(IDENTITY, 'PUT', 'http://localhost:8787/projects/p1?X=1', sign)

      expect(sign).toHaveBeenCalledWith('0xephemeral', `put:/projects/p1?x=1:${headers['x-identity-timestamp']}:${headers['x-identity-metadata']}`)
    })
  })
})
