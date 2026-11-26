import type { ElectronApplication, Page } from 'playwright';

/**
 * A valid 40-hex ethereum address used as the signed-in account in tests. The
 * single-sign-on client validates the address against `/^0x[a-fA-F0-9]{40}$/`
 * before persisting the identity, so this must be a well-formed address.
 */
export const MOCK_ADDRESS = '0x1234567890123456789012345678901234567890';

/**
 * A well-formed identity as returned by the auth server's
 * `GET /identities/:id` endpoint. `fetchIdentity` reads `authChain[0].payload`
 * as the signer address, and the SSO client only persists the identity when
 * `expiration` is in the future — so both must be present and valid.
 */
export function buildMockIdentity(address: string = MOCK_ADDRESS) {
  return {
    expiration: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString(),
    ephemeralIdentity: { address },
    authChain: [{ type: 'SIGNER', payload: address, signature: '' }],
  };
}

/** A stubbed HTTP reply. The optional body is serialized as JSON. */
export type StubResponse = { status: number; body?: unknown };

export type AuthMockOptions = {
  /** The requestId the stubbed `POST /requests` returns. Ignored if `requestsResponse` is set. */
  requestId?: string;
  /** The address the stubbed identity resolves to. Ignored if `identityResponse` is set. */
  address?: string;
  /** Override the whole `POST /requests` reply. Defaults to 200 `{ requestId }`. */
  requestsResponse?: StubResponse;
  /** Override the whole `GET /identities/:id` reply. Defaults to 200 `{ identity }`. */
  identityResponse?: StubResponse;
};

export type RecordedFetch = {
  url: string;
  method: string;
  body: string | null;
  verifier?: string | null;
};

/** Reads back what the installed mocks observed. */
export type AuthMockRecorder = {
  /** URLs the app asked the OS to open in the default browser. */
  openCalls: () => Promise<string[]>;
  /** Auth-server requests the stub answered. */
  fetchCalls: () => Promise<RecordedFetch[]>;
};

/** Records renderer request creation without reaching the relay. */
type StubWindow = {
  __e2eFetchCalls?: {
    url: string;
    method: string;
    body: string | null;
    verifier?: string | null;
  }[];
  __e2eFetchPatched?: boolean;
  __e2eRequestResponse?: StubResponse;
  fetch: typeof fetch;
};

function installRequestStub(page: Page, requests: StubResponse) {
  const script = (requests: StubResponse) => {
    const w = window as unknown as StubWindow;
    w.__e2eFetchCalls ??= [];
    w.__e2eRequestResponse = requests;
    if (w.__e2eFetchPatched) return;
    w.__e2eFetchPatched = true;

    const realFetch = w.fetch.bind(w);
    const reply = ({ status, body }: { status: number; body?: unknown }) =>
      new Response(body === undefined ? null : JSON.stringify(body), {
        status,
        headers: { 'Content-Type': 'application/json' },
      });

    w.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
      const method = (init?.method ?? 'GET').toUpperCase();

      if (url.includes('/requests') && method === 'POST') {
        w.__e2eFetchCalls!.push({ url, method, body: (init?.body as string) ?? null });
        return reply(w.__e2eRequestResponse!);
      }
      return realFetch(input, init);
    };
  };

  return Promise.all([page.addInitScript(script, requests), page.evaluate(script, requests)]);
}

type MainAuthStub = typeof globalThis & {
  __e2eIdentityResponse: StubResponse;
  __e2eIdentityCalls: RecordedFetch[];
};

async function installIdentityStub(electronApp: ElectronApplication, identity: StubResponse) {
  await electronApp.evaluate((_, reply) => {
    const main = globalThis as MainAuthStub;
    main.__e2eIdentityResponse = reply;
    main.__e2eIdentityCalls = [];
    const realFetch = main.fetch.bind(main);
    main.fetch = async (input, init) => {
      const url = new URL(input instanceof Request ? input.url : input);
      const relayHost = /^auth-api\.decentraland\.(org|zone|today)$/.test(url.hostname);
      const gateway = url.hostname === 'dcl.dev' && url.pathname.startsWith('/auth-api/');
      if ((relayHost || gateway) && /\/identities\/[^/]+$/.test(url.pathname)) {
        const request = new Request(input, init);
        main.__e2eIdentityCalls.push({
          url: url.href,
          method: request.method,
          body: null,
          verifier: request.headers.get('x-identity-verifier'),
        });
        const { status, body } = main.__e2eIdentityResponse;
        return new Response(body === undefined ? null : JSON.stringify(body), { status });
      }
      return realFetch(input, init);
    };
  }, identity);
}

/** Overrides the auth relay's reply for the next identity pickup. */
export function setIdentityResponse(electronApp: ElectronApplication, identity: StubResponse) {
  return electronApp.evaluate((_, response) => {
    (globalThis as MainAuthStub).__e2eIdentityResponse = response;
  }, identity);
}

/**
 * Mocks only the two external boundaries of sign-in, leaving every line of in-repo logic
 * to run for real: `AuthProvider` orchestration, the preload IPC bridge, main-process
 * deeplink parsing and dispatch, and identity persistence.
 *
 * **The browser handoff is stubbed in the main process.** `lib/auth.ts` calls
 * `window.open` (nothing sets `openBrowser`), which `security-restrictions.ts`'s
 * `setWindowOpenHandler` turns into `shell.openExternal` for allowlisted origins — and
 * both `decentraland.org` and `decentraland.zone` are on that allowlist. Stubbing
 * `shell.openExternal` therefore exercises the real renderer → handler → IPC → main
 * path, whereas patching `window.open` in the renderer would bypass all of it.
 *
 * Deliberately not stubbed: the `io(authServerUrl)` socket in `lib/auth.ts`. Only the
 * wallet-signature flow uses it, not the deeplink flow under test.
 */
export async function installAuthMocks(
  page: Page,
  electronApp: ElectronApplication,
  options: AuthMockOptions = {},
): Promise<AuthMockRecorder> {
  const requestId = options.requestId ?? 'e2e-request-id';
  const identity = buildMockIdentity(options.address ?? MOCK_ADDRESS);

  await installRequestStub(page, options.requestsResponse ?? { status: 200, body: { requestId } });
  await installIdentityStub(
    electronApp,
    options.identityResponse ?? { status: 200, body: { identity } },
  );

  const openCalls = await captureOpenExternal(electronApp);

  return {
    openCalls,
    fetchCalls: async () => [
      ...(await page.evaluate(() => (window as unknown as StubWindow).__e2eFetchCalls ?? [])),
      ...(await electronApp.evaluate(() => (globalThis as MainAuthStub).__e2eIdentityCalls)),
    ],
  };
}

/**
 * Stubs `shell.openExternal` in the main process.
 *
 * @remarks Only the injected electron module is reachable inside `evaluate` — the main process
 * is bundled ESM, so `require` is not defined there.
 * @returns a reader for the URLs the app asked the OS to open.
 */
export async function captureOpenExternal(
  electronApp: ElectronApplication,
): Promise<() => Promise<string[]>> {
  await electronApp.evaluate(({ shell }) => {
    const store = globalThis as unknown as { __e2eOpenExternalCalls?: string[] };
    store.__e2eOpenExternalCalls = [];
    shell.openExternal = async (url: string) => {
      store.__e2eOpenExternalCalls!.push(url);
    };
  });

  return () =>
    electronApp.evaluate(
      () =>
        (globalThis as unknown as { __e2eOpenExternalCalls?: string[] }).__e2eOpenExternalCalls ??
        [],
    );
}

/**
 * Fires a sign-in deeplink through the real macOS `open-url` entry point.
 *
 * @param authRequestId - Correlation id echoed by the auth dapp. `AuthProvider` completes
 * sign in only when it matches the app-generated id; omit it (or pass a foreign one) to
 * fire an uncorrelated link the app must reject.
 */
export async function fireSignInDeeplink(
  electronApp: ElectronApplication,
  identityId: string,
  authRequestId?: string,
): Promise<void> {
  const params = new URLSearchParams({ signin: identityId });
  if (authRequestId) {
    params.set('authRequestId', authRequestId);
  }
  await electronApp.evaluate(({ app }, url) => {
    app.emit('open-url', { preventDefault() {} }, url);
  }, `dcl-creator-hub://open?${params.toString()}`);
}

/**
 * Reads the locally-generated request id out of an opened auth-dapp URL.
 *
 * @returns the UUID v4 the app minted for the attempt, or `undefined` if the URL carries none.
 */
export function requestIdFromAuthUrl(url: string): string | undefined {
  return url.match(
    /\/requests\/([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})\?/i,
  )?.[1];
}
