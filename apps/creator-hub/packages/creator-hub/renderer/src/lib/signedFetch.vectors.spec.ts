// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { recoverMessageAddress } from 'viem';
import type { Hex } from 'viem';

import vectors from '../../tests/fixtures-signed-fetch-vectors.json';

import {
  type AuthChain,
  type AuthIdentity,
  createIdentity,
  createIdentityKeys,
  signPayload,
  signWithKeys,
} from './identity';
import { signedFetchFactory, signedHeaders } from './signedFetch';

/**
 * The signer of this app must give the same bytes as the packages it replaced.
 *
 * How the vectors were made: a Node script ran `@dcl/crypto` 3.7.0 and `decentraland-crypto-fetch`
 * 3.0.0 (the versions in `madeWith`) with the clock stopped at `clock.now`. It made each key from
 * the SHA-256 of its `label`, by giving that value to `createUnsafeIdentity` as its random bytes.
 * Then it recorded the result of `Authenticator.initializeAuthChain` for each identity,
 * `Authenticator.signPayload` for each payload, `signedHeaderFactory()` for each header case and
 * the `Request` that `signedFetchFactory({ fetch })` gave to `fetch` for each call. The cases are
 * the methods, paths, queries and metadata this app sends. `validSignature` records that
 * `Authenticator.validateSignature` accepted the chain.
 *
 * Time zone: the replaced packages add the minutes of an expiration to the local clock, and
 * `createIdentity` does the same. The script set `TZ=UTC` for its own process (`clock.timeZone`),
 * so the vectors do not depend on the zone of the machine that made them, and the renderer tests
 * run in UTC (`renderer/vite.config.js`). The `localTime` cases are the exception: the script made
 * each one in the zone it names, to record how the expiration moves when the clocks change.
 *
 * The keys are throwaway test keys made for the fixture. Nothing here uses a real wallet or a
 * server. The suite runs in Node so `Headers`, `Request` and `URL` are the platform ones.
 */
type IdentityVector = (typeof vectors.identities)[number];

const { throwawayTestKeys: keys } = vectors;

const stored = new Map(vectors.identities.map(vector => [vector.name, vector.identity]));

/** The identity as the app holds it: the stored JSON with `expiration` as a date. */
function identityNamed(name: string): AuthIdentity {
  const identity = stored.get(name)!;
  return { ...identity, expiration: new Date(identity.expiration) } as AuthIdentity;
}

const keysNamed = (name: string) => {
  const { label: _label, ...identity } = keys[name as keyof typeof keys];
  return identity;
};

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(vectors.clock.now);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('when the suite reads the local clock', () => {
  it('should be in the time zone of the vectors', () => {
    const longest = Math.max(...vectors.identities.map(vector => vector.minutes ?? 0));

    expect(vectors.clock.timeZone).toEqual('UTC');
    expect(new Date().getTimezoneOffset()).toBe(0);
    expect(new Date(vectors.clock.now + longest * 60_000).getTimezoneOffset()).toBe(0);
  });
});

describe('when making the keys of an identity', () => {
  it.each(Object.keys(keys))('should give the recorded keys and address for "%s"', name => {
    const expected = keysNamed(name);

    expect(createIdentityKeys(`0x${expected.privateKey}`)).toEqual(expected);
  });

  it('should make new keys each time when no private key is given', () => {
    const first = createIdentityKeys();
    const second = createIdentityKeys();

    expect(first.privateKey).toMatch(/^[0-9a-f]{64}$/);
    expect(first.publicKey).toMatch(/^[0-9a-f]{128}$/);
    expect(first.privateKey).not.toEqual(second.privateKey);
  });
});

describe('when making an identity', () => {
  const made = vectors.identities.filter(vector => 'ephemeralKey' in vector) as Extract<
    IdentityVector,
    { ephemeralKey: string }
  >[];

  it.each(made)('should give the recorded identity for "$name"', async vector => {
    const signer = vi.fn(async (message: string) =>
      vector.ownerSignsWith === 'owner key'
        ? signWithKeys(keysNamed('owner'), message)
        : vectors.contractWalletSignature,
    );

    const identity = await createIdentity(
      vector.ownerAddress,
      keysNamed(vector.ephemeralKey),
      vector.minutes,
      signer,
    );

    expect(signer).toHaveBeenCalledWith(vector.signedMessage);
    expect(JSON.stringify(identity)).toEqual(JSON.stringify(vector.identity));
  });
});

/**
 * The expiration is the local clock plus the minutes. When the clocks of the zone change in that
 * span, the expiration is not `minutes` after now: it moves by the change. The replaced packages
 * did the same, and each case records what they gave in its zone. The test changes the zone of its
 * own process, so it needs a pool that gives each worker a process (`forks`, the default).
 */
describe('when the clocks of the time zone change before the expiration', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it.each(vectors.localTime)(
    'should move the expiration by that change for "$name"',
    async vector => {
      vi.stubEnv('TZ', vector.timeZone);
      const minutesLater = vectors.clock.now + vector.minutes * 60_000;

      const { expiration } = await createIdentity(
        keysNamed('owner').address,
        keysNamed(vector.ephemeralKey),
        vector.minutes,
        async () => vectors.contractWalletSignature,
      );

      const change = expiration.getTimezoneOffset() - new Date().getTimezoneOffset();
      expect(change).toEqual(vector.shiftMinutes);
      expect(expiration.getTime() - minutesLater).toEqual(change * 60_000);
      expect(expiration.toISOString()).toEqual(vector.expiration);
    },
  );
});

describe('when signing a payload', () => {
  it.each(vectors.payloads)('should give the recorded chain for "$name"', async vector => {
    const chain = await signPayload(identityNamed(vector.identity), vector.payload);

    expect(JSON.stringify(chain)).toEqual(JSON.stringify(vector.authChain));
  });

  it('should leave the chain of the identity as it was', async () => {
    const identity = identityNamed('personal');
    const before = JSON.stringify(identity.authChain);

    await signPayload(identity, 'bafy');

    expect(JSON.stringify(identity.authChain)).toEqual(before);
  });
});

describe('when checking the signatures of a recorded chain', () => {
  const checked = vectors.payloads.filter(vector => 'validSignature' in vector);

  /** The address that signed each link after the first, in order. */
  async function signersOf(chain: AuthChain) {
    return Promise.all(
      chain
        .slice(1)
        .map(link =>
          recoverMessageAddress({ message: link.payload, signature: link.signature as Hex }),
        ),
    );
  }

  it.each(checked)('should find the wallet and the ephemeral key for "$name"', async vector => {
    const chain = vector.authChain as AuthChain;
    const ephemeralAddress = identityNamed(vector.identity).ephemeralIdentity.address;

    expect(await signersOf(chain)).toEqual([chain[0].payload, ephemeralAddress]);
  });

  it.each([1, 2])('should not find the signer when the payload of link %i changes', async index => {
    const chain = structuredClone(vectors.payloads[0].authChain) as AuthChain;
    const signers = await signersOf(chain);
    chain[index].payload += ' ';

    expect((await signersOf(chain))[index - 1]).not.toEqual(signers[index - 1]);
  });
});

describe('when making the headers of a signed request', () => {
  it.each(vectors.headers)('should give the recorded headers for "$name"', async vector => {
    const headers = await signedHeaders(
      identityNamed(vector.identity),
      vector.method,
      vector.path,
      vector.metadata,
      'init' in vector ? vector.init : undefined,
    );

    expect([...headers.entries()]).toEqual(vector.headers);
  });

  it.each(vectors.headerErrors)('should fail as before for "$name"', async vector => {
    const result = signedHeaders(
      identityNamed(vector.identity),
      vector.method,
      vector.path,
      vector.metadata,
    );

    expect(vector.throws).toEqual('TypeError');
    await expect(result).rejects.toBeInstanceOf(TypeError);
  });
});

describe('when making a signed request', () => {
  it.each(vectors.fetches)('should give the recorded request for "$name"', async vector => {
    const fetchMock = vi.fn(async (_input: Request) => new Response('{}'));
    const signedFetch = signedFetchFactory({ fetch: fetchMock as unknown as typeof fetch });
    const formBody = 'body' in vector.init && vector.init.body === 'form-data';
    const form = new FormData();
    form.append('title', 'My World');

    await signedFetch(typeof vector.input === 'string' ? vector.input : new URL(vector.input.url), {
      ...vector.init,
      ...(formBody ? { body: form } : {}),
      identity: identityNamed(vector.identity),
    } as RequestInit);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]).toHaveLength(1);
    const request = fetchMock.mock.calls[0][0];
    expect(request).toBeInstanceOf(Request);
    expect({
      url: request.url,
      method: request.method,
      headers: [...request.headers.entries()].filter(
        ([name]) => !formBody || name !== 'content-type',
      ),
      ...(formBody ? {} : { body: await request.text() }),
    }).toEqual(vector.request);
  });

  it('should give the call to fetch as it came when there is no identity', async () => {
    const fetchMock = vi.fn(async () => new Response('{}'));
    const signedFetch = signedFetchFactory({ fetch: fetchMock as unknown as typeof fetch });
    const init = { method: 'POST', body: '{}' };

    await signedFetch('https://worlds-content-server.example.org/worlds?limit=1', init);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [input, givenInit] = fetchMock.mock.calls[0] as unknown[];
    expect(input).toEqual('https://worlds-content-server.example.org/worlds?limit=1');
    expect(givenInit).toBe(init);
  });
});
