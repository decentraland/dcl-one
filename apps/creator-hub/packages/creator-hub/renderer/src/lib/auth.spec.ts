import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { deriveSignInRequestId } from './signInVerifier';

const ACCOUNT = '0x1111111111111111111111111111111111111111';
const AUTH_CHAIN = [{ type: 'SIGNER', payload: ACCOUNT, signature: '' }];

const getIdentityMock = vi.fn();
const ioMock = vi.fn();

vi.mock('@dcl/single-sign-on-client', () => ({
  localStorageGetIdentity: (...args: unknown[]) => getIdentityMock(...args),
  localStorageStoreIdentity: vi.fn(),
  localStorageClearIdentity: vi.fn(),
}));

vi.mock('socket.io-client', () => ({ io: (...args: unknown[]) => ioMock(...args) }));

const { AuthServerProvider, RelayError, SignInError } = await import('./auth');

function fakeSocket(outcome: object | undefined) {
  return {
    on: (event: string, listener: (msg: unknown) => void) => {
      if (event === 'connect') queueMicrotask(() => listener(undefined));
      if (event === 'outcome' && outcome) {
        queueMicrotask(() => listener({ requestId: 'req-1', ...outcome }));
      }
    },
    off: vi.fn(),
    disconnect: vi.fn(),
    timeout: vi.fn().mockReturnThis(),
    emitWithAck: vi.fn(async () => ({
      requestId: 'req-1',
      expiration: new Date(Date.now() + 50).toISOString(),
    })),
  };
}

const sendTransaction = () =>
  new AuthServerProvider().request({
    method: 'eth_sendTransaction',
    params: [{ to: ACCOUNT, data: '0x' }],
  });

describe('AuthServerProvider.request', () => {
  beforeEach(() => {
    AuthServerProvider.setOpenBrowser(vi.fn());
    AuthServerProvider.setAuthServerUrl('https://auth-api.decentraland.org');
    localStorage.setItem('auth-server-provider-address', ACCOUNT);
  });

  afterEach(() => {
    localStorage.clear();
    getIdentityMock.mockReset();
    ioMock.mockReset();
  });

  describe('when the relay is mounted under a gateway prefix', () => {
    beforeEach(() => {
      AuthServerProvider.setAuthServerUrl('https://dcl.dev/auth-api');
      getIdentityMock.mockReturnValue({ authChain: AUTH_CHAIN });
      ioMock.mockReturnValue(fakeSocket({ result: '0xhash' }));
    });

    it('should deliver the outcome via the prefixed Socket.IO transport', async () => {
      await expect(sendTransaction()).resolves.toBe('0xhash');
      expect(ioMock).toHaveBeenCalledWith('https://dcl.dev', {
        path: '/auth-api/socket.io',
        transports: ['websocket'],
      });
    });
  });

  describe('when there is no live identity', () => {
    beforeEach(() => {
      getIdentityMock.mockReturnValue(null);
    });

    it('should refuse with a no_identity error before opening a socket', async () => {
      await expect(sendTransaction()).rejects.toMatchObject({
        name: 'RelayError',
        reason: 'no_identity',
      });
      expect(ioMock).not.toHaveBeenCalled();
    });
  });

  describe('when there is a live identity', () => {
    beforeEach(() => {
      getIdentityMock.mockReturnValue({ authChain: AUTH_CHAIN });
    });

    describe('and the socket never connects', () => {
      let socket: ReturnType<typeof fakeSocket>;

      beforeEach(() => {
        vi.useFakeTimers();
        socket = fakeSocket(undefined);
        socket.on = vi.fn();
        ioMock.mockReturnValue(socket);
      });

      afterEach(() => vi.useRealTimers());

      it('should stop waiting and disconnect', async () => {
        const rejected = expect(sendTransaction()).rejects.toMatchObject({ reason: 'unavailable' });
        await vi.advanceTimersByTimeAsync(15_000);
        await rejected;
        expect(socket.disconnect).toHaveBeenCalledOnce();
      });
    });

    describe('and the socket reports a connection error', () => {
      let socket: ReturnType<typeof fakeSocket>;

      beforeEach(() => {
        vi.useFakeTimers();
        socket = fakeSocket(undefined);
        socket.on = (event, listener) => {
          if (event === 'connect_error')
            queueMicrotask(() => listener(new Error('connection failed')));
        };
        ioMock.mockReturnValue(socket);
      });

      afterEach(() => vi.useRealTimers());

      it('should immediately disconnect, clear the deadline and report the relay as unavailable', async () => {
        await expect(sendTransaction()).rejects.toMatchObject({ reason: 'unavailable' });
        expect(socket.disconnect).toHaveBeenCalledOnce();
        expect(vi.getTimerCount()).toBe(0);
        expect(socket.emitWithAck).not.toHaveBeenCalled();
      });
    });

    describe('and the acknowledgement times out', () => {
      let socket: ReturnType<typeof fakeSocket>;

      beforeEach(() => {
        socket = fakeSocket(undefined);
        socket.emitWithAck.mockRejectedValueOnce(new Error('operation has timed out'));
        ioMock.mockReturnValue(socket);
      });

      it('should disconnect and report the relay as unavailable', async () => {
        await expect(sendTransaction()).rejects.toMatchObject({ reason: 'unavailable' });
        expect(socket.timeout).toHaveBeenCalledWith(15_000);
        expect(socket.disconnect).toHaveBeenCalledOnce();
      });
    });

    describe('and the wallet returns a hash', () => {
      let socket: ReturnType<typeof fakeSocket>;

      beforeEach(() => {
        socket = fakeSocket({ result: '0xhash' });
        ioMock.mockReturnValue(socket);
      });

      it('should send only method, params and authChain to the server', async () => {
        await sendTransaction();

        expect(socket.emitWithAck).toHaveBeenCalledWith('request', {
          method: 'eth_sendTransaction',
          params: [{ to: ACCOUNT, data: '0x' }],
          authChain: AUTH_CHAIN,
        });
      });

      it('should use WebSocket on the relay host', async () => {
        await sendTransaction();
        expect(ioMock).toHaveBeenCalledWith('https://auth-api.decentraland.org', {
          transports: ['websocket'],
        });
      });

      it('should resolve with the hash', async () => {
        await expect(sendTransaction()).resolves.toBe('0xhash');
      });
    });

    describe('and the user rejects it in the auth dapp', () => {
      beforeEach(() => {
        ioMock.mockReturnValue(
          fakeSocket({ error: { code: -32003, message: 'Transaction rejected' } }),
        );
      });

      it('should reject with a rejected error', async () => {
        const error = await sendTransaction().catch(e => e);

        expect(error).toBeInstanceOf(RelayError);
        expect(error.reason).toBe('rejected');
      });
    });

    describe('and the wallet fails for another reason', () => {
      beforeEach(() => {
        ioMock.mockReturnValue(fakeSocket({ error: { code: -32000, message: 'boom' } }));
      });

      it('should reject with the original error', async () => {
        await expect(sendTransaction()).rejects.toEqual({ code: -32000, message: 'boom' });
      });
    });

    describe('and the request expires without an outcome', () => {
      beforeEach(() => {
        ioMock.mockReturnValue(fakeSocket(undefined));
      });

      it('should reject with an expired error', async () => {
        await expect(sendTransaction()).rejects.toMatchObject({
          name: 'RelayError',
          reason: 'expired',
        });
      });
    });
  });
});

const AUTH_SERVER_URL = 'https://auth-api.example.com';
const IDENTITY_ID = 'f0963d93-f520-4612-9714-8d0d30f7ae6d';
const VERIFIER = 'dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk';

describe('when fetching a deep-link sign in identity', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    AuthServerProvider.setAuthServerUrl(AUTH_SERVER_URL);
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe('and the auth server returns the identity', () => {
    beforeEach(() => {
      fetchMock.mockResolvedValueOnce(
        new Response(JSON.stringify({ identity: { authChain: [{ payload: '0xABC' }] } }), {
          status: 200,
        }),
      );
    });

    it('should send the verifier header', async () => {
      await AuthServerProvider.fetchIdentity(IDENTITY_ID, VERIFIER);
      expect(fetchMock).toHaveBeenCalledWith(`${AUTH_SERVER_URL}/identities/${IDENTITY_ID}`, {
        headers: { 'x-identity-verifier': VERIFIER },
      });
    });

    it('should return the lowercased signer', async () => {
      const { signer } = await AuthServerProvider.fetchIdentity(IDENTITY_ID, VERIFIER);
      expect(signer).toBe('0xabc');
    });
  });

  describe('and the auth server rejects the verifier', () => {
    beforeEach(() => {
      fetchMock.mockResolvedValueOnce(new Response('{}', { status: 401 }));
    });

    it('should throw a sign in error with the verifier reason', async () => {
      await expect(AuthServerProvider.fetchIdentity(IDENTITY_ID, VERIFIER)).rejects.toMatchObject(
        new SignInError('verifier', 'Sign in identity belongs to another sign in request'),
      );
    });
  });
});

describe('when creating a deep-link sign in request', () => {
  let request: { requestId: string; verifier: string };

  beforeEach(async () => {
    request = await AuthServerProvider.createSignInRequest();
  });

  it('should return a request id derived from the returned verifier', async () => {
    expect(request.requestId).toBe(await deriveSignInRequestId(request.verifier));
  });
});
