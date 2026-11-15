import { type AuthIdentity, signPayload } from './identity';

// The header names of a signed request. The services read exactly these.
export const AUTH_CHAIN_HEADER_PREFIX = 'x-identity-auth-chain-';
export const AUTH_TIMESTAMP_HEADER = 'x-identity-timestamp';
export const AUTH_METADATA_HEADER = 'x-identity-metadata';

export type Metadata = Record<string, unknown>;

export type SignedRequestInit = RequestInit & {
  identity?: AuthIdentity;
  metadata?: Metadata;
};

/**
 * The headers of a signed request: `init` plus one header for each link of the chain, the
 * timestamp and the metadata.
 *
 * The signed text is `<method>:<path>:<timestamp>:<metadata>`. Method and path go in lower case.
 * The metadata is signed and sent as the same string, so the signature covers its exact bytes.
 */
export async function signedHeaders(
  identity: Pick<AuthIdentity, 'ephemeralIdentity' | 'authChain'>,
  method: string,
  path: string,
  metadata: Metadata = {},
  init?: HeadersInit,
): Promise<Headers> {
  const headers = new Headers(init);
  const timestamp = String(Date.now());
  const data = JSON.stringify(metadata);
  const payload = [method.toLowerCase(), path.toLowerCase(), timestamp, data].join(':');
  const chain = await signPayload(identity, payload);
  chain.forEach((link, index) => {
    headers.set(AUTH_CHAIN_HEADER_PREFIX + index, JSON.stringify(link));
  });
  headers.set(AUTH_TIMESTAMP_HEADER, timestamp);
  headers.set(AUTH_METADATA_HEADER, data);
  return headers;
}

/**
 * Makes a `fetch` that signs the request when `init.identity` is given and is a plain `fetch`
 * when it is not. The signed path is the path of the URL, without the query.
 *
 * It keeps the `fetch` that exists when the factory runs. A test that replaces the global `fetch`
 * later must give its own with `options.fetch`.
 */
export function signedFetchFactory(options: { fetch?: typeof fetch } = {}) {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  return async function signedFetch(
    input: string | URL,
    init?: SignedRequestInit,
  ): Promise<Response> {
    if (!init?.identity) return fetchImpl(input, init);
    const { identity, metadata, ...requestInit } = init;
    const url = typeof input === 'string' ? new URL(input) : input;
    const headers = await signedHeaders(
      identity,
      init.method || 'GET',
      url.pathname,
      metadata || {},
      requestInit.headers,
    );
    return fetchImpl(new Request(url.toString(), { ...requestInit, headers }));
  };
}

export const signedFetch = signedFetchFactory();
