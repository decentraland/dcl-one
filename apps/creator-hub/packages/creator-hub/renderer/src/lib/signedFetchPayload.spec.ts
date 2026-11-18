import { describe, expect, it } from 'vitest';

import { createIdentity, createIdentityKeys, signWithKeys } from './identity';
import { signedHeaders } from './signedFetch';

/**
 * Pins the ADR-44 payload format this app signs.
 *
 * The services check the signature against the metadata bytes as delivered. So the metadata goes
 * into the signed text with its case: a world access password or a community id with capitals in
 * it must not be changed. Only the method and the path go in lower case.
 */
describe('when signing a request with metadata', () => {
  /** The payload the signature covers is the last auth-chain link's `payload`. */
  async function signedPayloadFor(
    method: string,
    path: string,
    metadata: Record<string, unknown>,
  ): Promise<string> {
    const ephemeralIdentity = createIdentityKeys();
    const throwawayAccount = createIdentityKeys();
    const identity = await createIdentity(
      throwawayAccount.address,
      ephemeralIdentity,
      10,
      message => signWithKeys(throwawayAccount, message),
    );

    const headers = await signedHeaders(identity, method, path, metadata);
    const links: { payload: string }[] = [];
    headers.forEach((value, name) => {
      if (name.startsWith('x-identity-auth-chain-')) {
        links.push(JSON.parse(value));
      }
    });

    return links[links.length - 1].payload;
  }

  it('should leave the metadata bytes exactly as they will be delivered', async () => {
    // The world access password: the flow that broke. The dialog never normalizes its case.
    const metadata = { type: 'shared-secret', secret: 'MyPassWord123' };

    const payload = await signedPayloadFor(
      'POST',
      '/world/foo.dcl.eth/permissions/access',
      metadata,
    );

    expect(payload.endsWith(JSON.stringify(metadata))).toBe(true);
  });

  it('should lowercase the method and the path', async () => {
    const payload = await signedPayloadFor('POST', '/World/Foo/Permissions', {});

    expect(payload.startsWith('post:/world/foo/permissions:')).toBe(true);
  });

  it('should distinguish metadata differing only in case', async () => {
    // If the two gave one signed text, a field with changed case would pass with the same signature.
    const path = '/world/foo.dcl.eth/permissions/access';

    const lower = await signedPayloadFor('POST', path, {
      type: 'shared-secret',
      secret: 'mypassword123',
    });
    const mixed = await signedPayloadFor('POST', path, {
      type: 'shared-secret',
      secret: 'MyPassWord123',
    });

    const metadataOf = (payload: string) => payload.split(':').slice(3).join(':');

    expect(metadataOf(lower)).not.toEqual(metadataOf(mixed));
  });
});
