import type { Hex } from 'viem'
import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts'

// A deterministic, obviously fake key for the test wallet. Never a real account.
export const TEST_PRIVATE_KEY: Hex = `0x${'11'.repeat(32)}`
export const TEST_ADDRESS = privateKeyToAccount(TEST_PRIVATE_KEY).address.toLowerCase()
export const POLYGON_AMOY = 80002

export type TestSession = { address: string; identityJson: string }

let cached: Promise<TestSession> | undefined

/** A real signed identity for the test wallet, in the shape the site keeps, so signed requests are well formed. */
export function testSession(): Promise<TestSession> {
  cached ??= (async () => {
    const user = privateKeyToAccount(TEST_PRIVATE_KEY)
    const ephemeralKey = generatePrivateKey()
    const ephemeral = privateKeyToAccount(ephemeralKey)
    const expiration = new Date(Date.now() + 31 * 24 * 60 * 60_000).toISOString()
    const payload = `Decentraland Login\nEphemeral address: ${ephemeral.address}\nExpiration: ${expiration}`
    const identity = {
      signer: user.address,
      ephemeral: { address: ephemeral.address, publicKey: ephemeral.publicKey, privateKey: ephemeralKey },
      expiration,
      authChain: [
        { type: 'SIGNER', payload: user.address, signature: '' },
        { type: 'ECDSA_EPHEMERAL', payload, signature: await user.signMessage({ message: payload }) }
      ]
    }
    return { address: TEST_ADDRESS, identityJson: JSON.stringify(identity) }
  })()
  return cached
}

/**
 * Seeds the session the site restores on load and installs a mock EIP-1193 provider, so the pages sign in through
 * their production code path, as a browser wallet. Requests go to node (`__e2eWallet`), where the fake chain signs
 * with the test key and logs transactions.
 */
export function sessionInitScript(session: TestSession, chainId: number): string {
  return `(() => {
    localStorage.setItem('dcl:auth:identity:v1', ${JSON.stringify(session.identityJson)});
    const CHAIN = '0x${chainId.toString(16)}';
    const handle = (method, params) => {
      if (method === 'eth_chainId') return Promise.resolve(CHAIN);
      if (method === 'net_version') return Promise.resolve(String(${chainId}));
      return window.__e2eWallet(method, params || []);
    };
    const eth = {
      isMetaMask: true,
      chainId: CHAIN,
      request: ({ method, params }) => handle(method, params),
      enable: () => handle('eth_requestAccounts'),
      on: () => {}, removeListener: () => {}, removeAllListeners: () => {}
    };
    eth.send = (m, p) => (typeof m === 'string' ? handle(m, p) : handle(m.method, m.params));
    eth.sendAsync = (payload, cb) => handle(payload.method, payload.params)
      .then(result => cb(null, { id: payload.id, jsonrpc: '2.0', result }))
      .catch(err => cb(err));
    window.ethereum = eth;
  })()`
}

/**
 * Request interception can't see WebSockets, so the social RPC (friends) would reach the real service.
 * Off-origin sockets fail at once instead, the way an unreachable host does; the dev server's own socket is kept.
 */
export const OFFLINE_WEBSOCKET_SCRIPT = `(() => {
  const RealWebSocket = window.WebSocket;
  class OfflineWebSocket extends EventTarget {
    constructor(url) {
      super();
      this.url = String(url);
      this.readyState = 3;
      setTimeout(() => {
        const error = new Event('error');
        this.onerror && this.onerror(error);
        this.dispatchEvent(error);
        const close = new CloseEvent('close', { code: 1006 });
        this.onclose && this.onclose(close);
        this.dispatchEvent(close);
      }, 0);
    }
    send() {}
    close() {}
  }
  function E2EWebSocket(url, protocols) {
    return new URL(String(url), location.href).hostname === location.hostname
      ? new RealWebSocket(url, protocols)
      : new OfflineWebSocket(url);
  }
  Object.assign(E2EWebSocket, { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 });
  E2EWebSocket.prototype = RealWebSocket.prototype;
  window.WebSocket = E2EWebSocket;
})()`
