import { beforeEach, describe, expect, it, vi } from 'vitest';

const PEER = 'https://peer.example.org';

const fetchMock = vi.fn();

vi.mock('/@/config', () => ({
  config: { get: () => PEER },
}));

vi.mock('/shared/fetch', () => ({
  fetch: (...args: unknown[]) => fetchMock(...args),
}));

const { DCLNames } = await import('./ens');

const page = (names: string[], totalAmount: number) => ({
  ok: true,
  status: 200,
  json: () => Promise.resolve({ elements: names.map(name => ({ name })), totalAmount }),
});

describe('DCLNames.fetchNames', () => {
  let names: string[];

  beforeEach(() => {
    fetchMock.mockReset();
  });

  describe('when the owner holds fewer NAMEs than a page', () => {
    beforeEach(async () => {
      fetchMock.mockResolvedValueOnce(page(['Alice', 'bob'], 2));
      names = await new DCLNames().fetchNames('0xAbC0000000000000000000000000000000000001');
    });

    it('should ask the content server lambdas for the lowercased address, first page', () => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
      expect(fetchMock).toHaveBeenCalledWith(
        `${PEER}/lambdas/users/0xabc0000000000000000000000000000000000001/names?pageSize=1000&pageNum=1`,
        undefined,
        10000,
      );
    });

    it('should answer each NAME as minted under .dcl.eth', () => {
      expect(names).toEqual(['Alice.dcl.eth', 'bob.dcl.eth']);
    });
  });

  describe('when the owner holds more NAMEs than a page', () => {
    beforeEach(async () => {
      const full = Array.from({ length: 1000 }, (_, i) => `n${i}`);
      fetchMock.mockResolvedValueOnce(page(full, 1001));
      fetchMock.mockResolvedValueOnce(page(['last'], 1001));
      names = await new DCLNames().fetchNames('0x1');
    });

    it('should walk the pages until the total', () => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
      expect(fetchMock.mock.calls[1][0]).toBe(
        `${PEER}/lambdas/users/0x1/names?pageSize=1000&pageNum=2`,
      );
      expect(names).toHaveLength(1001);
      expect(names[1000]).toBe('last.dcl.eth');
    });
  });

  describe('when a short page comes before the announced total', () => {
    beforeEach(async () => {
      const full = Array.from({ length: 1000 }, (_, i) => `n${i}`);
      fetchMock.mockResolvedValueOnce(page(full, 5000));
      fetchMock.mockResolvedValueOnce(page(['a', 'b', 'c'], 5000));
      names = await new DCLNames().fetchNames('0x1');
    });

    it('should end the walk on the short page', () => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
      expect(names).toHaveLength(1003);
    });
  });

  describe('when the content server cannot answer', () => {
    let cancel: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      cancel = vi.fn().mockResolvedValue(undefined);
      fetchMock.mockResolvedValueOnce({ ok: false, status: 503, body: { cancel } });
    });

    it('should fail rather than answer no NAMEs, releasing the unread body', async () => {
      await expect(new DCLNames().fetchNames('0x1')).rejects.toThrow('503');
      expect(cancel).toHaveBeenCalledTimes(1);
    });
  });
});

describe('DCLNames.fetchNamesOwners', () => {
  let owners: Record<string, string>;

  beforeEach(() => {
    fetchMock.mockReset();
  });

  describe('when one NAME is held and another is not', () => {
    let cancel: ReturnType<typeof vi.fn>;

    beforeEach(async () => {
      cancel = vi.fn().mockResolvedValue(undefined);
      fetchMock.mockImplementation((url: string) =>
        Promise.resolve(
          url.includes('/names/alice/')
            ? { ok: true, status: 200, json: () => Promise.resolve({ owner: '0xa1' }) }
            : { ok: false, status: 404, body: { cancel } },
        ),
      );
      owners = await new DCLNames().fetchNamesOwners(['Alice', 'alice', 'nobody']);
    });

    it('should ask once per distinct NAME', () => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });

    it('should release the body of the 404', () => {
      expect(cancel).toHaveBeenCalledTimes(1);
    });

    it('should ask the owner route once per NAME, lowercased', () => {
      expect(fetchMock).toHaveBeenCalledWith(`${PEER}/lambdas/names/alice/owner`, undefined, 10000);
      expect(fetchMock).toHaveBeenCalledWith(
        `${PEER}/lambdas/names/nobody/owner`,
        undefined,
        10000,
      );
    });

    it('should key the holder by the lowercased NAME and leave out a NAME nobody holds', () => {
      expect(owners).toEqual({ alice: '0xa1' });
    });
  });

  describe('when an owner lookup fails', () => {
    let cancel: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      cancel = vi.fn().mockResolvedValue(undefined);
      fetchMock.mockResolvedValue({ ok: false, status: 502, body: { cancel } });
    });

    it('should fail the whole read rather than answer the NAME unheld, releasing the body', async () => {
      await expect(new DCLNames().fetchNamesOwners(['alice'])).rejects.toThrow('502');
      expect(cancel).toHaveBeenCalledTimes(1);
    });
  });

  describe('when a lookup fails while others wait their turn', () => {
    beforeEach(() => {
      fetchMock.mockResolvedValue({
        ok: false,
        status: 502,
        body: { cancel: () => Promise.resolve() },
      });
    });

    it('should not send the lookups still queued behind it', async () => {
      const labels = Array.from({ length: 20 }, (_, i) => `n${i}`);
      await expect(new DCLNames().fetchNamesOwners(labels)).rejects.toThrow('502');
      // The first eight start together; none queued behind them is sent.
      expect(fetchMock.mock.calls.length).toBeLessThanOrEqual(8);
    });
  });

  describe('when no NAMEs are asked', () => {
    beforeEach(async () => {
      owners = await new DCLNames().fetchNamesOwners([]);
    });

    it('should ask nothing', () => {
      expect(fetchMock).not.toHaveBeenCalled();
      expect(owners).toEqual({});
    });
  });
});
