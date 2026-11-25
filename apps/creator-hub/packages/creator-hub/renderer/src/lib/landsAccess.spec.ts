import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const fetchMock = vi.fn();

vi.mock('/shared/fetch', () => ({ fetch: (...args: unknown[]) => fetchMock(...args) }));

// No LAND_MANAGER_SUBGRAPH: the environment reads the LAND list from its peer's index.
const CONFIG: Record<string, string> = {
  PEER_URL: 'https://peer.example.org',
  LAND_REGISTRY_ADDRESS: '0xland',
  ESTATE_REGISTRY_ADDRESS: '0xestate',
  LAND_MANAGER_SUBGRAPH: '',
};

vi.mock('/@/config', () => ({ config: { get: (key: string) => CONFIG[key] ?? '' } }));

const { Lands, LandType, RoleType } = await import('./land');

const parcel = (x: number, y: number, owner = '0xme') => ({
  x: String(x),
  y: String(y),
  tokenId: `token-${x}-${y}`,
  owner: { address: owner },
  operator: null,
  updateOperator: null,
  data: null,
});

const estate = (id: string, owner = '0xme') => ({
  id,
  owner: { address: owner },
  operator: null,
  updateOperator: null,
  size: 1,
  parcels: [{ x: '0', y: '9', tokenId: 'token-0-9', updateOperator: null }],
  data: { name: 'Plaza', description: null },
});

const EMPTY = {
  tenantParcels: [],
  tenantEstates: [],
  lessorParcels: [],
  lessorEstates: [],
  ownerParcels: [],
  ownerEstates: [],
  updateOperatorParcels: [],
  updateOperatorEstates: [],
  approvedParcels: [],
  approvedEstates: [],
  ownerAuthorizations: [],
  operatorAuthorizations: [],
  grantorParcels: [],
  grantorEstates: [],
};

/** A real JSON response, as the peer sends it. */
const answer = (lists: Record<string, unknown>) =>
  new Response(JSON.stringify({ ...EMPTY, ...lists }), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });

/** Serves a fresh response per request (a body reads once). */
const serve = (make: () => Response) => fetchMock.mockImplementation(() => Promise.resolve(make()));

/** A body stream that sends `first`, then never ends; `cancelled` flips when it is cancelled. */
function stalledStream(first: string) {
  const state = { cancelled: false };
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(first));
    },
    cancel() {
      state.cancelled = true;
    },
  });
  return { stream, state };
}

const grant = (tokenAddress: string, isApproved: boolean) => ({
  type: 'UpdateManager',
  owner: { address: '0xowner' },
  isApproved,
  tokenAddress,
  timestamp: '1700000000000002',
});

describe('Lands.fetchLand from the land-authz index', () => {
  beforeEach(() => {
    fetchMock.mockReset();
  });

  describe('when it asks for the LAND list', () => {
    beforeEach(() => {
      serve(() => answer({}));
    });

    it("should POST the wallet's rentals to the peer's lands-access route, once", async () => {
      await new Lands().fetchLand('0xME', ['1'], ['2']);

      expect(fetchMock).toHaveBeenCalledTimes(1);
      const [url, init, timeout] = fetchMock.mock.calls[0];
      expect(url).toBe('https://peer.example.org/lambdas/users/0xme/lands-access');
      expect(init.method).toBe('POST');
      expect(JSON.parse(init.body)).toEqual({ tenantTokenIds: ['1'], lessorTokenIds: ['2'] });
      expect(timeout).toBe(30_000);
    });
  });

  describe('when the peer cannot answer', () => {
    let body: ReturnType<typeof stalledStream>;

    beforeEach(() => {
      body = stalledStream('{"error":');
      serve(() => new Response(body.stream, { status: 503 }));
    });

    it('should fail on the status at once, releasing the unread body', async () => {
      await expect(new Lands().fetchLand('0xme')).rejects.toThrow('503');
      await Promise.resolve();
      expect(body.state.cancelled).toBe(true);
    });
  });

  describe('when the headers arrive but the body stalls', () => {
    let body: ReturnType<typeof stalledStream>;

    beforeEach(() => {
      vi.useFakeTimers();
      body = stalledStream('{"ownerParcels":');
      serve(() => new Response(body.stream, { status: 200 }));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('should fail within the deadline and cancel the stream itself', async () => {
      const read = new Lands().fetchLand('0xme');
      const failed = expect(read).rejects.toThrow('did not finish');
      await vi.advanceTimersByTimeAsync(30_000);
      await failed;
      expect(body.state.cancelled).toBe(true);
    });
  });

  describe('when the index answers without the grantor lists', () => {
    beforeEach(() => {
      const { grantorParcels: _p, grantorEstates: _e, ...partial } = EMPTY;
      serve(
        () =>
          new Response(
            JSON.stringify({ ...partial, operatorAuthorizations: [grant('0xland', true)] }),
            { status: 200 },
          ),
      );
    });

    it('should refuse the answer, with no second request to another source', async () => {
      await expect(new Lands().fetchLand('0xme')).rejects.toThrow('grantorParcels');
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the wallet owns land and an owner made it an UpdateManager', () => {
    beforeEach(() => {
      serve(() =>
        answer({
          ownerParcels: [parcel(1, 1)],
          ownerEstates: [estate('7')],
          operatorAuthorizations: [grant('0xland', true)],
          grantorParcels: [parcel(2, 2, '0xowner')],
        }),
      );
    });

    it('should list its own land as owned and the grantor parcels as managed, from one request', async () => {
      const [lands] = await new Lands().fetchLand('0xme');

      expect(Object.fromEntries(lands.map(land => [land.id, land.role]))).toEqual({
        '1,1': RoleType.OWNER,
        '7': RoleType.OWNER,
        '2,2': RoleType.MANAGER,
      });
      expect(lands.find(land => land.id === '2,2')?.deployable).toBe(true);
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the grant was revoked', () => {
    beforeEach(() => {
      serve(() => answer({ operatorAuthorizations: [grant('0xland', false)] }));
    });

    it('should list no managed land', async () => {
      const [lands] = await new Lands().fetchLand('0xme');

      expect(lands).toEqual([]);
    });
  });

  describe('when the wallet made someone an UpdateManager of its LAND', () => {
    beforeEach(() => {
      serve(() =>
        answer({
          ownerParcels: [parcel(1, 1)],
          ownerAuthorizations: [
            {
              type: 'UpdateManager',
              operator: '0xmanager',
              isApproved: true,
              tokenAddress: '0xland',
              timestamp: '0',
            },
          ],
        }),
      );
    });

    it('should name it on the parcels, even with an unknown block time', async () => {
      const [lands, authorizations] = await new Lands().fetchLand('0xme');

      expect(authorizations).toEqual([{ address: '0xmanager', type: LandType.PARCEL }]);
      expect(lands[0].operators).toEqual(['0xmanager']);
    });
  });
});
