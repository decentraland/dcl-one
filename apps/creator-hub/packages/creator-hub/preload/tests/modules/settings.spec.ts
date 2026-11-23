import { beforeEach, describe, expect, it, vi } from 'vitest';

// A config file in memory, read and written through the same IPC calls as the real one.
const disk = vi.hoisted(() => ({ config: { settings: {} as Record<string, unknown> } }));
vi.mock('../../src/services/ipc', () => ({
  invoke: vi.fn(async (channel: string, value?: unknown) => {
    if (channel === 'config.getConfig') return structuredClone(disk.config);
    if (channel === 'config.writeConfig')
      disk.config = structuredClone(value) as typeof disk.config;
  }),
}));

import {
  getAnalyticsWatchlist,
  setAnalyticsWatchlist,
  updateAiSharingSettings,
  updateAppSettings,
} from '../../src/modules/settings';

const WALLET = '0x1234567890abcdef1234567890abcdef12345678';
const denied = { consent: 'denied', version: 1, decidedAt: '2026-11-18T00:00:00Z' };

beforeEach(() => {
  disk.config = { settings: { aiDataSharing: { [WALLET]: denied } } };
});

describe('when a screen saves the settings with an older copy of the sharing keys', () => {
  it('should keep the sharing decision on disk', async () => {
    await updateAppSettings({ aiAssistant: true, aiDataSharing: {} } as never);

    expect(disk.config.settings).toMatchObject({
      aiAssistant: true,
      aiDataSharing: { [WALLET]: denied },
    });
  });
});

describe('when the sharing keys and other settings are saved at once', () => {
  it('should keep both changes', async () => {
    const granted = { ...denied, consent: 'granted' };
    await Promise.all([
      updateAiSharingSettings({ aiDataSharing: { [WALLET]: granted } } as never),
      updateAppSettings({ aiAssistant: false } as never),
    ]);

    expect(disk.config.settings).toMatchObject({
      aiAssistant: false,
      aiDataSharing: { [WALLET]: granted },
    });
  });
});

describe('when a wallet saves its Analytics watchlist', () => {
  const OTHER_WALLET = '0x0000000000000000000000000000000000000001';
  const PLACE = 'world:example.dcl.eth@0,0';

  beforeEach(async () => {
    disk.config.settings.analyticsWatchlist = {
      [OTHER_WALLET]: { version: 1, placeIds: ['land:1,1'] },
    };
    await setAnalyticsWatchlist(WALLET.toUpperCase().replace('0X', '0x'), [PLACE]);
  });

  it('should read it back under any case of the address', async () => {
    expect(await getAnalyticsWatchlist(WALLET)).toEqual([PLACE]);
  });

  it("should keep the other wallets' lists and the other settings", async () => {
    expect(await getAnalyticsWatchlist(OTHER_WALLET)).toEqual(['land:1,1']);
    expect(disk.config.settings.aiDataSharing).toEqual({ [WALLET]: denied });
  });

  it('should survive a screen saving an older copy of the settings', async () => {
    await updateAppSettings({ aiAssistant: true, analyticsWatchlist: {} } as never);

    expect(await getAnalyticsWatchlist(WALLET)).toEqual([PLACE]);
  });
});

describe('when nothing usable is stored for the Analytics watchlist', () => {
  it('should read an empty list', async () => {
    expect(await getAnalyticsWatchlist(WALLET)).toEqual([]);

    disk.config.settings.analyticsWatchlist = 'corrupt';
    expect(await getAnalyticsWatchlist(WALLET)).toEqual([]);
  });
});
