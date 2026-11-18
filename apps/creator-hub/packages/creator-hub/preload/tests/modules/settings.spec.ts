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

import { updateAiSharingSettings, updateAppSettings } from '../../src/modules/settings';

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
