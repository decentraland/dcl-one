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

  describe('when the content server cannot answer', () => {
    beforeEach(() => {
      fetchMock.mockResolvedValueOnce({ ok: false, status: 503 });
    });

    it('should fail rather than answer no NAMEs', async () => {
      await expect(new DCLNames().fetchNames('0x1')).rejects.toThrow('503');
    });
  });
});

describe('DCLNames.fetchNamesOwners', () => {
  let owners: Record<string, string>;

  beforeEach(() => {
    fetchMock.mockReset();
  });

  describe('when one NAME is held and another is not', () => {
    beforeEach(async () => {
      fetchMock.mockImplementation((url: string) =>
        Promise.resolve(
          url.includes('/names/alice/')
            ? { ok: true, status: 200, json: () => Promise.resolve({ owner: '0xa1' }) }
            : { ok: false, status: 404 },
        ),
      );
      owners = await new DCLNames().fetchNamesOwners(['Alice', 'nobody']);
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
    beforeEach(() => {
      fetchMock.mockResolvedValue({ ok: false, status: 502 });
    });

    it('should fail the whole read rather than answer the NAME unheld', async () => {
      await expect(new DCLNames().fetchNamesOwners(['alice'])).rejects.toThrow('502');
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
