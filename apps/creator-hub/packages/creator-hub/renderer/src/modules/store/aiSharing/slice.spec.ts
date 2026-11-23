import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  AI_SHARING_CONSENT_VERSION,
  type AiSharingConsent,
  type AiTurnRecord,
} from '/shared/types/ai-sharing';

import { ai, aiSharing, settings as settingsPreload } from '#preload';
import * as api from '../../../lib/aiSharingApi';
import { AuthServerProvider } from '../../../lib/auth';
import { createTestStore, type TestStore } from '../../../../tests/utils/testStore';
import { actions as aiActions } from '../ai';
import { FeatureFlag } from '../featureFlags/types';
import { actions, canManageSharing, selectSharingView, shouldShareTurn } from './slice';

vi.mock('../../../lib/aiSharingApi', () => ({
  isConfigured: vi.fn(() => true),
  MAX_RECORDS_PER_UPLOAD: 20,
  MAX_UPLOAD_BYTES: 900_000,
  getConsent: vi.fn(),
  putConsent: vi.fn(),
  uploadTurns: vi.fn(),
  listTurns: vi.fn(),
  eraseAll: vi.fn(),
  eraseTurn: vi.fn(),
  eraseConversation: vi.fn(),
  putRating: vi.fn(),
  listDigests: vi.fn(),
}));
vi.mock('../../../lib/auth', () => ({ AuthServerProvider: { getAccount: vi.fn() } }));

const ADDRESS = '0x1234567890abcdef1234567890abcdef12345678';
const ok = <T>(data: T, status = 200) => ({ ok: true as const, status, data });
const fail = (status: number, body?: object) => ({
  ok: false as const,
  status,
  error: 'nope',
  body,
});
const granted = {
  consent: 'granted' as const,
  version: AI_SHARING_CONSENT_VERSION,
  decidedAt: '2026-11-12T00:00:00Z',
  delivered: true,
};
const record = (id: string) => ({ recordId: id, address: ADDRESS }) as AiTurnRecord;
const NOW = '2026-11-12T00:00:00Z';
const LATER = '2026-11-13T00:00:00Z';
const serverSays = (isGranted: boolean, decidedAt: string | null) => ({
  granted: isGranted,
  version: decidedAt === null ? null : AI_SHARING_CONSENT_VERSION,
  decidedAt,
});

function setup({
  flag = true,
  consent,
  messages = [],
}: {
  flag?: boolean;
  consent?: AiSharingConsent;
  messages?: Array<{
    role: 'user' | 'assistant';
    done: boolean;
    error?: string;
    recordId?: string;
  }>;
} = {}): TestStore {
  const store = createTestStore();
  store.dispatch({
    type: 'featureFlags/fetch/fulfilled',
    payload: { flags: { [FeatureFlag.AI_PROMPT_SHARING]: flag }, variants: {} },
  });
  store.dispatch({
    type: 'config/updateSettings/fulfilled',
    meta: { arg: { aiAssistant: true } },
  });
  store.dispatch({
    type: 'config/updateAiSharingSettings/fulfilled',
    meta: { arg: { aiDataSharing: consent ? { [ADDRESS]: consent } : undefined } },
  });
  store.dispatch(
    aiActions.setSessionState({
      sessions: [{ id: 's1', title: 'chat', updatedAt: 1 }],
      currentSessionId: 's1',
      messages: messages.map((m, i) => ({ id: `m${i}`, parts: [], ...m })),
    }),
  );
  return store;
}

const settled = () => new Promise(resolve => setTimeout(resolve, 0));
const backoffOf = (store: TestStore, wallet = ADDRESS) =>
  store.getState().aiSharing.backoff[wallet] ?? { failures: 0, nextUploadAt: 0 };

beforeEach(() => {
  vi.mocked(AuthServerProvider.getAccount).mockReturnValue(ADDRESS);
  vi.mocked(api.isConfigured).mockReturnValue(true);
  vi.mocked(aiSharing.peek).mockResolvedValue([]);
  vi.mocked(aiSharing.list).mockResolvedValue([]);
});

describe('when deciding what the chat shows', () => {
  const replied = [{ role: 'assistant' as const, done: true }];

  it('should show nothing while the feature flag is off', () => {
    const store = setup({ flag: false, messages: replied });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
  });

  it('should show nothing when the service is not configured', () => {
    vi.mocked(api.isConfigured).mockReturnValue(false);
    const store = setup({ messages: replied });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
  });

  it('should wait for a first finished reply before asking', () => {
    const store = setup({ messages: [{ role: 'user', done: true }] });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
  });

  it('should ask a signed-in creator who was never asked', () => {
    const store = setup({ messages: replied });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('ask');
  });

  it('should tell a signed-out creator to sign in instead', () => {
    const store = setup({ messages: replied });

    expect(selectSharingView(store.getState(), null).status).toBe('signin');
  });

  it('should stop asking after "not now" for this run', () => {
    const store = setup({ messages: replied });
    store.dispatch(actions.notNow());

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
  });

  it('should never ask again someone who said no', () => {
    const store = setup({ messages: replied, consent: { ...granted, consent: 'denied' } });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
  });

  it('should ask again someone who said yes to an older version of the text', () => {
    const store = setup({ messages: replied, consent: { ...granted, version: 0 } });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('ask');
  });

  it('should show the chip while sharing, with the conversation exclusion', async () => {
    const store = setup({ messages: replied, consent: granted });
    await store.dispatch(aiActions.setConversationShared(false));

    expect(selectSharingView(store.getState(), ADDRESS)).toEqual({
      status: 'on',
      conversationExcluded: true,
      conversationHasShared: false,
    });
  });

  it('should tell the chat when the open conversation already shared replies', () => {
    const store = setup({
      consent: granted,
      messages: [{ role: 'assistant', done: true, recordId: 'r1' }],
    });

    expect(selectSharingView(store.getState(), ADDRESS).conversationHasShared).toBe(true);
  });
});

describe('when a turn is sent', () => {
  it('should share it only with current consent, signed in, and not excluded', async () => {
    const store = setup({ consent: granted });
    expect(shouldShareTurn(store.getState())).toBe(true);

    await store.dispatch(aiActions.setConversationShared(false));
    expect(shouldShareTurn(store.getState())).toBe(false);
  });

  it('should not share it when signed out', () => {
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(undefined as never);
    const store = setup({ consent: granted });

    expect(shouldShareTurn(store.getState())).toBe(false);
  });

  it('should not share it without consent', () => {
    expect(shouldShareTurn(setup().getState())).toBe(false);
  });
});

describe('when the chat sends a turn', () => {
  const sendIn = async (store: TestStore) => {
    store.dispatch({
      type: 'workspace/runProject/fulfilled',
      payload: { path: '/scene', title: 's' },
    });
    vi.mocked(ai.isBusy).mockResolvedValue(false);
    await store.dispatch(aiActions.send({ text: 'add a door' }));
    return vi.mocked(ai.send).mock.calls[0][1];
  };

  it('should ask main to share it, with counts only', async () => {
    const params = await sendIn(setup({ consent: granted }));

    expect(params.share).toEqual({
      wallet: ADDRESS,
      turnIndex: 0,
      selectedEntityCount: 0,
      mentionedAssetCount: 0,
    });
  });

  it('should not ask main to share it without consent', async () => {
    const params = await sendIn(setup());

    expect(params.share).toBeUndefined();
  });

  describe('and the chat has no id yet', () => {
    const newChat = () => {
      const store = setup({ consent: granted });
      store.dispatch(
        aiActions.setSessionState({ sessions: [], currentSessionId: '', messages: [] }),
      );
      return store;
    };

    it('should give the chat an id and send every turn of it under that id', async () => {
      const store = newChat();
      const first = await sendIn(store);
      store.dispatch(aiActions.setBusy(false));
      await store.dispatch(aiActions.send({ text: 'and a window' }));
      const second = vi.mocked(ai.send).mock.calls[1][1];

      expect(first.sessionId).toMatch(/\S/);
      expect(second.sessionId).toBe(first.sessionId);
      expect(store.getState().ai.currentSessionId).toBe(first.sessionId);
    });

    it('should exclude and erase the chat by the id its turns were sent under', async () => {
      vi.mocked(api.eraseConversation).mockResolvedValue(ok({ deleted: 1 }));
      const store = newChat();
      const { sessionId } = await sendIn(store);
      store.dispatch(aiActions.applyEvent({ kind: 'started', turnId: 't1' }));
      store.dispatch(
        aiActions.applyEvent({
          kind: 'done',
          turnId: 't1',
          ok: true,
          mutations: 0,
          recordId: 'r1',
        }),
      );

      store.dispatch(aiActions.setConversationShared(false));

      expect(aiSharing.setConversationShared).toHaveBeenCalledWith(sessionId, false);
      await vi.waitFor(() =>
        expect(api.eraseConversation).toHaveBeenCalledWith(ADDRESS, sessionId, expect.any(String)),
      );
    });
  });
});

describe('when the creator decides', () => {
  it('should save a yes and send it to the server with the time it was made', async () => {
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(true, NOW)));
    const store = setup();

    await store.dispatch(actions.decide(true));

    const saved = store.getState().workspace.settings.aiDataSharing![ADDRESS];
    expect(saved).toMatchObject({
      consent: 'granted',
      version: AI_SHARING_CONSENT_VERSION,
      delivered: true,
    });
    expect(api.putConsent).toHaveBeenCalledWith(ADDRESS, {
      granted: true,
      version: AI_SHARING_CONSENT_VERSION,
      decidedAt: saved.decidedAt,
    });
    expect(store.getState().aiSharing.syncedWallet).toBe(ADDRESS);
  });

  it('should empty the local queue on a no', async () => {
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(false, NOW)));
    const store = setup({ consent: granted });

    await store.dispatch(actions.decide(false));

    expect(aiSharing.purge).toHaveBeenCalledWith(ADDRESS);
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.consent).toBe('denied');
  });

  it('should keep the local answer, unsynced, when the server cannot be reached', async () => {
    vi.mocked(api.putConsent).mockResolvedValue(fail(0));
    const store = setup();

    await store.dispatch(actions.decide(true));

    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]).toMatchObject({
      consent: 'granted',
      delivered: false,
    });
    expect(store.getState().aiSharing.syncedWallet).toBeNull();
  });
});

describe('when settling consent with the server before uploading', () => {
  it('should upload without re-sending a decision the server already has', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(api.putConsent).not.toHaveBeenCalled();
    expect(store.getState().aiSharing.syncedWallet).toBe(ADDRESS);
  });

  it('should send a decision that never arrived, keeping its original time', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, null)));
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    const store = setup({ consent: { ...granted, delivered: false } });

    await store.dispatch(actions.flush());

    expect(api.putConsent).toHaveBeenCalledWith(ADDRESS, {
      granted: true,
      version: AI_SHARING_CONSENT_VERSION,
      decidedAt: granted.decidedAt,
    });
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.delivered).toBe(true);
  });

  it('should send an undelivered decision even when the server holds a later one', async () => {
    // The creator's click here is their latest action on this machine; the server, which
    // orders by receive time, decides with the 409 rule whether it stands.
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, LATER)));
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    const store = setup({ consent: { ...granted, delivered: false } });

    await store.dispatch(actions.flush());

    expect(api.putConsent).toHaveBeenCalled();
  });

  it('should adopt the withdrawal when the server refuses a grant older than it', async () => {
    vi.mocked(api.getConsent)
      .mockResolvedValueOnce(ok(serverSays(false, LATER)))
      .mockResolvedValueOnce(ok(serverSays(false, LATER)));
    vi.mocked(api.putConsent).mockResolvedValue(fail(409, { currentVersion: 1 }));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    const store = setup({ consent: { ...granted, delivered: false } });

    await store.dispatch(actions.flush());

    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]).toMatchObject({
      consent: 'denied',
      decidedAt: LATER,
    });
    expect(aiSharing.purge).toHaveBeenCalledWith(ADDRESS);
    expect(api.uploadTurns).not.toHaveBeenCalled();
  });

  it('should adopt a later decision made on another machine', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, LATER)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(api.putConsent).not.toHaveBeenCalled();
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]).toMatchObject({
      consent: 'denied',
      decidedAt: LATER,
    });
    expect(api.uploadTurns).not.toHaveBeenCalled();
  });

  it('should re-send a delivered decision the server has lost', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, null)));
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(api.putConsent).toHaveBeenCalledWith(
      ADDRESS,
      expect.objectContaining({ granted: true, decidedAt: granted.decidedAt }),
    );
  });

  it('should send a local withdrawal that never reached the server', async () => {
    const denied = { ...granted, consent: 'denied' as const, decidedAt: LATER, delivered: false };
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(false, LATER)));
    const store = setup({ consent: denied });

    await store.dispatch(actions.flush());

    expect(api.putConsent).toHaveBeenCalledWith(
      ADDRESS,
      expect.objectContaining({ granted: false }),
    );
    expect(api.uploadTurns).not.toHaveBeenCalled();
  });

  it('should stop offering sharing when the server has a newer consent text', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, null)));
    vi.mocked(api.putConsent).mockResolvedValue(
      fail(409, { currentVersion: AI_SHARING_CONSENT_VERSION + 1 }),
    );
    const store = setup({
      consent: { ...granted, delivered: false },
      messages: [{ role: 'assistant', done: true }],
    });

    await store.dispatch(actions.flush());

    expect(store.getState().aiSharing.versionMismatch).toBe(true);
    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
    expect(shouldShareTurn(store.getState())).toBe(false);
  });

  it('should wait and retry when the server cannot be reached', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(fail(0));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(api.uploadTurns).not.toHaveBeenCalled();
    expect(backoffOf(store).nextUploadAt).toBeGreaterThan(Date.now());
  });
});

describe('when uploading', () => {
  beforeEach(() => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
  });

  it('should upload and acknowledge each batch', async () => {
    vi.mocked(aiSharing.peek)
      .mockResolvedValueOnce([record('a'), record('b')])
      .mockResolvedValueOnce([]);
    vi.mocked(api.uploadTurns).mockResolvedValue(ok({ accepted: 2, rejected: [] }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.peek).toHaveBeenCalledWith(ADDRESS, 20);
    expect(aiSharing.ack).toHaveBeenCalledWith(['a', 'b']);
  });

  it('should keep each request under the body cap, however big the records are', async () => {
    const big = (id: string) => ({ ...record(id), padding: 'x'.repeat(120_000) }) as AiTurnRecord;
    const queue = Array.from({ length: 20 }, (_, i) => big(`r${i}`));
    vi.mocked(aiSharing.peek).mockImplementation(async () => queue);
    vi.mocked(aiSharing.ack).mockImplementation(async ids => {
      for (const id of ids)
        queue.splice(
          queue.findIndex(r => r.recordId === id),
          1,
        );
    });
    vi.mocked(api.uploadTurns).mockResolvedValue(ok({ accepted: 1, rejected: [] }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    const sizes = vi
      .mocked(api.uploadTurns)
      .mock.calls.map(([, records]) => JSON.stringify({ records }).length);
    expect(Math.max(...sizes)).toBeLessThanOrEqual(api.MAX_UPLOAD_BYTES);
    expect(queue).toEqual([]);
  });

  it('should upload nothing without consent', async () => {
    const store = setup();

    await store.dispatch(actions.flush());

    expect(aiSharing.peek).not.toHaveBeenCalled();
    expect(api.uploadTurns).not.toHaveBeenCalled();
  });

  it('should upload nothing while the flag is off', async () => {
    const store = setup({ flag: false, consent: granted });

    await store.dispatch(actions.flush());

    expect(api.uploadTurns).not.toHaveBeenCalled();
  });

  it.each([0, 401, 429, 503])('should keep the batch and back off on %s', async status => {
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(fail(status));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).not.toHaveBeenCalled();
    expect(backoffOf(store).nextUploadAt).toBeGreaterThan(Date.now());

    await store.dispatch(actions.flush());
    expect(api.uploadTurns).toHaveBeenCalledTimes(1);
  });

  it('should acknowledge records the service accepted and rejected alike', async () => {
    vi.mocked(aiSharing.peek)
      .mockResolvedValueOnce([record('a'), record('bad')])
      .mockResolvedValue([]);
    vi.mocked(api.uploadTurns).mockResolvedValue(
      ok({ accepted: 1, rejected: [{ recordId: 'bad', reason: 'prompt too long' }] }),
    );
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).toHaveBeenCalledWith(['a', 'bad']);
  });

  it('should keep the records when the service calls the request malformed', async () => {
    // A 400 means the request or its signature headers are wrong (a client bug), not the
    // records; dropping them would empty the queue on every such bug.
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(fail(400));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).not.toHaveBeenCalled();
    expect(backoffOf(store).nextUploadAt).toBeGreaterThan(Date.now());
  });

  it('should split a batch the service finds too large', async () => {
    const queue = [record('a'), record('b')];
    vi.mocked(aiSharing.peek).mockImplementation(async () => [...queue]);
    vi.mocked(aiSharing.ack).mockImplementation(async ids => {
      for (const id of ids)
        queue.splice(
          queue.findIndex(r => r.recordId === id),
          1,
        );
    });
    vi.mocked(api.uploadTurns).mockImplementation(async (_wallet, records) =>
      records.length > 1 ? fail(413) : ok({ accepted: 1, rejected: [] }),
    );
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(vi.mocked(api.uploadTurns).mock.calls.map(([, r]) => r.length)).toEqual([2, 1, 1]);
    expect(queue).toEqual([]);
  });

  it('should stop and settle consent again when the server says it has none', async () => {
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(fail(403));
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).not.toHaveBeenCalled();
    expect(store.getState().aiSharing.syncedWallet).toBeNull();
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.consent).toBe('granted');
  });

  it('should stop for good when the server has a newer consent text', async () => {
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(
      fail(403, { currentVersion: AI_SHARING_CONSENT_VERSION + 1 }),
    );
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());
    await store.dispatch(actions.flush());

    expect(store.getState().aiSharing.versionMismatch).toBe(true);
    expect(api.uploadTurns).toHaveBeenCalledTimes(1);
  });

  it('should not run two uploads at once', async () => {
    let release: () => void = () => {};
    vi.mocked(aiSharing.peek)
      .mockReturnValueOnce(new Promise(r => (release = () => r([]))) as never)
      .mockResolvedValue([]);
    const store = setup({ consent: granted });

    const first = store.dispatch(actions.flush());
    await settled();
    await store.dispatch(actions.flush());
    release();
    await first;

    expect(aiSharing.peek).toHaveBeenCalledTimes(1);
  });
});

describe('when deleting everything shared', () => {
  it('should delete on the server, empty the queue and turn sharing off', async () => {
    vi.mocked(api.eraseAll).mockResolvedValue(ok({ deleted: 7 }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.deleteEverything());

    expect(api.eraseAll).toHaveBeenCalledWith(
      ADDRESS,
      expect.stringMatching(/^\d{4}-\d{2}-\d{2}T/),
    );
    expect(api.putConsent).not.toHaveBeenCalled();

    expect(aiSharing.purge).toHaveBeenCalledWith(ADDRESS);
    expect(store.getState().aiSharing).toMatchObject({ deleteStatus: 'deleted', deletedCount: 7 });
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.consent).toBe('denied');
  });

  it('should stop sharing here even when the server delete fails, and say it failed', async () => {
    vi.mocked(api.eraseAll).mockResolvedValue(fail(500));
    const store = setup({ consent: granted });

    await store.dispatch(actions.deleteEverything());

    expect(store.getState().aiSharing.deleteStatus).toBe('failed');
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]).toMatchObject({
      consent: 'denied',
      delivered: false,
    });
    expect(aiSharing.purge).toHaveBeenCalledWith(ADDRESS);
  });

  it('should save the "no" before emptying the queue', async () => {
    let consentWhenPurged: unknown;
    const store = setup({ consent: granted });
    vi.mocked(aiSharing.purge).mockImplementation(async () => {
      consentWhenPurged = store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.consent;
    });
    vi.mocked(api.eraseAll).mockResolvedValue(ok({ deleted: 1 }));

    await store.dispatch(actions.deleteEverything());

    expect(consentWhenPurged).toBe('denied');
  });
});

describe('when another wallet signs in on the same machine', () => {
  const OTHER = '0x000000000000000000000000000000000000beef';

  it('should not treat it as having agreed', () => {
    const store = setup({ consent: granted, messages: [{ role: 'assistant', done: true }] });
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(OTHER);

    expect(shouldShareTurn(store.getState())).toBe(false);
    expect(selectSharingView(store.getState(), OTHER).status).toBe('ask');
  });

  it("should never send the first wallet's decision as its own", async () => {
    const store = setup({ consent: granted });
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(OTHER);
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, null)));

    await store.dispatch(actions.flush());

    expect(api.putConsent).not.toHaveBeenCalled();
    expect(api.uploadTurns).not.toHaveBeenCalled();
  });

  it("should keep each wallet's decision apart", async () => {
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(false, NOW)));
    const store = setup({ consent: granted });
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(OTHER);

    await store.dispatch(actions.decide(false));

    const byWallet = store.getState().workspace.settings.aiDataSharing!;
    expect(byWallet[ADDRESS].consent).toBe('granted');
    expect(byWallet[OTHER].consent).toBe('denied');
    expect(aiSharing.purge).toHaveBeenCalledWith(OTHER);
  });

  it('should stop an upload in progress when the wallet changes', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockImplementation(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(OTHER);
      return ok({ accepted: 1, rejected: [] });
    });
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(api.uploadTurns).toHaveBeenCalledTimes(1);
  });

  it('should keep records the service rejects for a wallet mismatch', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a'), record('b')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(
      ok({ accepted: 1, rejected: [{ recordId: 'b', reason: 'address_mismatch' }] }),
    );
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).toHaveBeenCalledWith(['a']);
    expect(api.uploadTurns).toHaveBeenCalledTimes(1);
  });
});

describe('when the creator withdraws while an upload runs', () => {
  it('should not send the next batch', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(false, LATER)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    const store = setup({ consent: granted });
    vi.mocked(api.uploadTurns).mockImplementation(async () => {
      await store.dispatch(actions.decide(false));
      return ok({ accepted: 1, rejected: [] });
    });

    await store.dispatch(actions.flush());

    expect(api.uploadTurns).toHaveBeenCalledTimes(1);
  });
});

describe('when the service asks to wait', () => {
  it('should wait at least as long as Retry-After says', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue({ ...fail(503), retryAfterSeconds: 3600 });
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(backoffOf(store).nextUploadAt).toBeGreaterThanOrEqual(Date.now() + 3599 * 1000);
  });

  it('should wait no longer than the longest backoff, whatever Retry-After says', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue({ ...fail(503), retryAfterSeconds: 86_400 });
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(backoffOf(store).nextUploadAt).toBeLessThanOrEqual(Date.now() + 3_600_000);
  });

  it('should honour Retry-After on a failed conversation erasure', async () => {
    vi.mocked(api.eraseConversation).mockResolvedValue({ ...fail(503), retryAfterSeconds: 1800 });
    const store = setup({ consent: granted });

    await store.dispatch(actions.eraseConversation({ conversationId: 's1', recordIds: [] }));

    expect(backoffOf(store).nextUploadAt).toBeGreaterThanOrEqual(Date.now() + 1799 * 1000);
  });
});

describe('when the service runs an older consent text than this app', () => {
  it('should stop offering sharing instead of asking again and again', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(false, null)));
    vi.mocked(api.putConsent).mockResolvedValue(
      fail(409, { currentVersion: AI_SHARING_CONSENT_VERSION - 1 }),
    );
    const store = setup({ consent: { ...granted, delivered: false } });

    await store.dispatch(actions.flush());

    expect(store.getState().aiSharing.versionMismatch).toBe(true);
    expect(api.getConsent).toHaveBeenCalledTimes(1);
  });
});

describe('when the feature flag is off but a decision is stored', () => {
  it('should still deliver a withdrawal', async () => {
    const denied = { ...granted, consent: 'denied' as const, decidedAt: LATER, delivered: false };
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(api.putConsent).mockResolvedValue(ok(serverSays(false, LATER)));
    const store = setup({ flag: false, consent: denied });

    await store.dispatch(actions.flush());

    expect(api.putConsent).toHaveBeenCalledWith(
      ADDRESS,
      expect.objectContaining({ granted: false }),
    );
  });

  it('should keep the Settings controls for it', () => {
    const store = setup({ flag: false, consent: granted });

    expect(canManageSharing(store.getState(), ADDRESS)).toBe(true);
    expect(canManageSharing(store.getState(), '0x000000000000000000000000000000000000beef')).toBe(
      false,
    );
  });
});

describe('when looking at what was shared', () => {
  const turn = (id: string) => ({ recordId: id }) as api.AiSharingStoredTurn;

  it('should load pages and append the next one', async () => {
    vi.mocked(api.listTurns)
      .mockResolvedValueOnce(ok({ records: [turn('a')], next: 'c1' }))
      .mockResolvedValueOnce(ok({ records: [turn('b')], next: null }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.loadSharedTurns({ reset: true }));
    await store.dispatch(actions.loadSharedTurns({ reset: false }));

    expect(api.listTurns).toHaveBeenLastCalledWith(ADDRESS, 'c1');
    expect(store.getState().aiSharing.shared).toMatchObject({
      status: 'loaded',
      next: null,
      turns: [{ recordId: 'a' }, { recordId: 'b' }],
    });
  });

  it('should erase one turn and leave sharing on', async () => {
    vi.mocked(api.listTurns).mockResolvedValue(ok({ records: [turn('a'), turn('b')], next: null }));
    vi.mocked(api.eraseTurn).mockResolvedValue(ok({ deleted: 1 }));
    const store = setup({ consent: granted });
    await store.dispatch(actions.loadSharedTurns({ reset: true }));

    await store.dispatch(actions.eraseSharedTurn('a'));

    expect(api.eraseTurn).toHaveBeenCalledWith(ADDRESS, 'a');
    expect(store.getState().aiSharing.shared.turns.map(t => t.recordId)).toEqual(['b']);
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.consent).toBe('granted');
  });
});

describe('when the first reply was stopped', () => {
  it('should not count it as a finished reply for the consent card', () => {
    const store = setup({
      messages: [{ role: 'assistant', done: true, cancelled: true } as never],
    });

    expect(selectSharingView(store.getState(), ADDRESS).status).toBe('hidden');
  });
});

describe('when the service rate-limits an upload', () => {
  it('should wait as long as Retry-After says on a 429', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue({ ...fail(429), retryAfterSeconds: 600 });
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).not.toHaveBeenCalled();
    expect(backoffOf(store).nextUploadAt).toBeGreaterThanOrEqual(Date.now() + 599_000);
  });
});

describe('when a "yes" given offline meets a feature that was turned off', () => {
  it('should not send it', async () => {
    const store = setup({ flag: false, consent: { ...granted, delivered: false } });

    await store.dispatch(actions.flush());

    expect(api.getConsent).not.toHaveBeenCalled();
    expect(api.putConsent).not.toHaveBeenCalled();
  });
});

describe('when a page of shared turns arrives late', () => {
  it('should drop it if everything was deleted meanwhile', async () => {
    let answer: (value: unknown) => void = () => {};
    vi.mocked(api.listTurns).mockReturnValue(new Promise(r => (answer = r)) as never);
    vi.mocked(api.eraseAll).mockResolvedValue(ok({ deleted: 1 }));
    const store = setup({ consent: granted });

    const loading = store.dispatch(actions.loadSharedTurns({ reset: true }));
    await store.dispatch(actions.deleteEverything());
    answer(ok({ records: [{ recordId: 'gone' }], next: null }));
    await loading;

    expect(store.getState().aiSharing.shared.turns).toEqual([]);
  });
});

describe('when reading the local queue', () => {
  it("should ask main for the signed-in wallet's records only", async () => {
    const store = setup({ consent: granted });

    await store.dispatch(actions.refreshQueue());

    expect(aiSharing.list).toHaveBeenCalledWith(ADDRESS);
  });
});

describe('when a withdrawal does not get through', () => {
  it('should forget the sync after a failed erase, so the next upload run retries it', async () => {
    vi.mocked(api.eraseAll).mockResolvedValue(fail(503));
    const store = setup({ consent: granted });
    store.dispatch(actions.setSyncedWallet(ADDRESS));

    await store.dispatch(actions.deleteEverything());

    expect(store.getState().aiSharing.syncedWallet).toBeNull();
    expect(store.getState().aiSharing.deleteStatus).toBe('failed');
  });

  it('should never turn a refused "no" back into the service\'s "yes"', async () => {
    vi.mocked(api.putConsent).mockResolvedValue(fail(409));
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    const store = setup({ consent: granted });

    await store.dispatch(actions.decide(false));

    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]).toMatchObject({
      consent: 'denied',
      delivered: false,
    });
    expect(api.getConsent).not.toHaveBeenCalled();
  });
});

const pendingOf = (store: TestStore) =>
  store.getState().workspace.settings.aiSharingPending?.[ADDRESS];

describe('when a conversation is marked as not shared', () => {
  const sharedReplies = [
    { role: 'user' as const, done: true },
    { role: 'assistant' as const, done: true, recordId: 'r1' },
  ];

  beforeEach(() => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
  });

  it('should erase what it already shared on the service', async () => {
    vi.mocked(api.eraseConversation).mockResolvedValue(ok({ deleted: 1 }));
    const store = setup({ consent: granted, messages: sharedReplies });

    store.dispatch(aiActions.setConversationShared(false));

    await vi.waitFor(() =>
      expect(api.eraseConversation).toHaveBeenCalledWith(ADDRESS, 's1', expect.any(String)),
    );
    await vi.waitFor(() => expect(pendingOf(store)).toBeUndefined());
    expect(aiSharing.setConversationShared).toHaveBeenCalledWith('s1', false);
  });

  it('should ask nothing of the service when the conversation shared nothing', async () => {
    const store = setup({ consent: granted, messages: [{ role: 'assistant', done: true }] });

    store.dispatch(aiActions.setConversationShared(false));
    await settled();

    expect(api.eraseConversation).not.toHaveBeenCalled();
    expect(pendingOf(store)).toBeUndefined();
  });

  it('should keep the erasure when the service fails, and send it again, same date, later', async () => {
    vi.mocked(api.eraseConversation)
      .mockResolvedValueOnce(fail(503))
      .mockResolvedValueOnce(ok({ deleted: 2 }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.eraseConversation({ conversationId: 's1', recordIds: ['r1'] }));

    const waiting = pendingOf(store)?.erasures;
    expect(waiting).toEqual([{ conversationId: 's1', decidedAt: expect.any(String) }]);
    expect(backoffOf(store).nextUploadAt).toBeGreaterThan(Date.now());

    store.dispatch(actions.uploadSucceeded({ wallet: ADDRESS }));
    await store.dispatch(actions.flush());

    expect(api.eraseConversation).toHaveBeenLastCalledWith(ADDRESS, 's1', waiting![0].decidedAt);
    expect(pendingOf(store)).toBeUndefined();
  });

  it('should keep trying after sharing was turned off, as withdrawals do', async () => {
    vi.mocked(api.eraseConversation).mockResolvedValue(ok({ deleted: 1 }));
    const store = setup({ consent: { ...granted, consent: 'denied' } });
    store.dispatch({
      type: 'config/updateAiSharingSettings/fulfilled',
      meta: {
        arg: {
          aiSharingPending: { [ADDRESS]: { erasures: [{ conversationId: 's9', decidedAt: NOW }] } },
        },
      },
    });

    await store.dispatch(actions.flush());

    expect(api.eraseConversation).toHaveBeenCalledWith(ADDRESS, 's9', NOW);
    expect(pendingOf(store)).toBeUndefined();
  });

  it("should drop the conversation's ratings that were still waiting", async () => {
    vi.mocked(api.eraseConversation).mockResolvedValue(fail(503));
    vi.mocked(api.putRating).mockResolvedValue(fail(503));
    const store = setup({ consent: granted });
    await store.dispatch(actions.rateTurn({ recordId: 'r1', rating: 'up' }));
    await store.dispatch(actions.rateTurn({ recordId: 'r2', rating: 'down' }));

    await store.dispatch(actions.eraseConversation({ conversationId: 's1', recordIds: ['r1'] }));

    expect(pendingOf(store)?.ratings).toEqual({ r2: 'down' });
  });
});

describe('when rating a shared reply', () => {
  beforeEach(() => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
  });

  it('should send it once the queued turns are uploaded', async () => {
    const order: string[] = [];
    vi.mocked(aiSharing.peek)
      .mockResolvedValueOnce([record('r1')])
      .mockResolvedValueOnce([]);
    vi.mocked(api.uploadTurns).mockImplementation(async () => {
      order.push('upload');
      return ok({ accepted: 1, rejected: [] });
    });
    vi.mocked(api.putRating).mockImplementation(async (_w, recordId, rating) => {
      order.push('rate');
      return ok({ recordId, rating });
    });
    const store = setup({ consent: granted });

    await store.dispatch(actions.rateTurn({ recordId: 'r1', rating: 'up' }));

    expect(api.putRating).toHaveBeenCalledWith(ADDRESS, 'r1', 'up');
    expect(order).toEqual(['upload', 'rate']);
    expect(pendingOf(store)).toBeUndefined();
  });

  it('should keep it and retry when the service fails', async () => {
    vi.mocked(api.putRating)
      .mockResolvedValueOnce(fail(503))
      .mockResolvedValueOnce(ok({ recordId: 'r1', rating: null }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.rateTurn({ recordId: 'r1', rating: null }));
    expect(pendingOf(store)?.ratings).toEqual({ r1: null });

    store.dispatch(actions.uploadSucceeded({ wallet: ADDRESS }));
    await store.dispatch(actions.flush());

    expect(api.putRating).toHaveBeenCalledTimes(2);
    expect(pendingOf(store)).toBeUndefined();
  });

  it('should drop a rating for a record the service does not have', async () => {
    vi.mocked(api.putRating).mockResolvedValue(fail(404));
    const store = setup({ consent: granted });

    await store.dispatch(actions.rateTurn({ recordId: 'gone', rating: 'down' }));

    expect(pendingOf(store)).toBeUndefined();
    expect(backoffOf(store).failures).toBe(0);
  });

  it('should send nothing without consent', async () => {
    const store = setup({ consent: { ...granted, consent: 'denied' } });

    await store.dispatch(actions.rateTurn({ recordId: 'r1', rating: 'up' }));

    expect(api.putRating).not.toHaveBeenCalled();
    expect(pendingOf(store)).toBeUndefined();
  });

  it('should save the rating on the reply and skip replies that were not shared', async () => {
    vi.mocked(api.putRating).mockResolvedValue(ok({ recordId: 'r1', rating: 'up' }));
    const store = setup({
      consent: granted,
      messages: [
        { role: 'assistant', done: true, recordId: 'r1' },
        { role: 'assistant', done: true },
      ],
    });

    store.dispatch(aiActions.rateTurn({ id: 'm0', rating: 'up' }));
    store.dispatch(aiActions.rateTurn({ id: 'm1', rating: 'up' }));

    expect(store.getState().ai.messages.map(m => m.rating)).toEqual(['up', undefined]);
    await vi.waitFor(() => expect(api.putRating).toHaveBeenCalledWith(ADDRESS, 'r1', 'up'));
    expect(api.putRating).toHaveBeenCalledTimes(1);
  });
});

describe('when looking at the summaries of what was shared', () => {
  const digest = (id: string) =>
    ({
      conversationId: id,
      goal: 'a door that opens',
      outcome: 'achieved',
      frictions: [],
      createdAt: NOW,
    }) as api.AiSharingDigest;

  it('should load pages and append the next one', async () => {
    vi.mocked(api.listDigests)
      .mockResolvedValueOnce(ok({ digests: [digest('a')], next: 'c1' }))
      .mockResolvedValueOnce(ok({ digests: [digest('b')], next: null }));
    const store = setup({ consent: granted });

    await store.dispatch(actions.loadDigests({ reset: true }));
    await store.dispatch(actions.loadDigests({ reset: false }));

    expect(api.listDigests).toHaveBeenLastCalledWith(ADDRESS, 'c1');
    expect(store.getState().aiSharing.digests).toMatchObject({
      status: 'loaded',
      next: null,
      digests: [{ conversationId: 'a' }, { conversationId: 'b' }],
    });
  });

  it('should say when they could not be loaded', async () => {
    vi.mocked(api.listDigests).mockResolvedValue(fail(503));
    const store = setup({ consent: granted });

    await store.dispatch(actions.loadDigests({ reset: true }));

    expect(store.getState().aiSharing.digests.status).toBe('failed');
  });

  it('should drop a page that arrives after everything was deleted', async () => {
    let answer: (value: unknown) => void = () => {};
    vi.mocked(api.listDigests).mockReturnValue(new Promise(r => (answer = r)) as never);
    vi.mocked(api.eraseAll).mockResolvedValue(ok({ deleted: 1 }));
    const store = setup({ consent: granted });

    const loading = store.dispatch(actions.loadDigests({ reset: true }));
    await store.dispatch(actions.deleteEverything());
    answer(ok({ digests: [digest('gone')], next: null }));
    await loading;

    expect(store.getState().aiSharing.digests.digests).toEqual([]);
  });
});

describe('when the creator changes their mind before the first answer arrives', () => {
  it('should send both decisions in order and keep the last one', async () => {
    let answerFirst: (value: unknown) => void = () => {};
    vi.mocked(api.putConsent)
      .mockReturnValueOnce(new Promise(r => (answerFirst = r)) as never)
      .mockImplementationOnce(async (_w, d) => ok(serverSays(d.granted, d.decidedAt)));
    const store = setup({ consent: { ...granted, consent: 'denied', delivered: true } });

    const yes = store.dispatch(actions.decide(true));
    await vi.waitFor(() => expect(api.putConsent).toHaveBeenCalledTimes(1));
    const no = store.dispatch(actions.decide(false));
    await settled();

    // The "no" waits for the "yes" to reach the service, so the service keeps the "no".
    expect(api.putConsent).toHaveBeenCalledTimes(1);
    answerFirst(ok(serverSays(true, NOW)));
    await Promise.all([yes, no]);

    expect(vi.mocked(api.putConsent).mock.calls.map(([, d]) => d.granted)).toEqual([true, false]);
    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]).toMatchObject({
      consent: 'denied',
      delivered: true,
    });
  });
});

describe('when another screen saves the settings with an older copy', () => {
  it('should keep the sharing decision the store holds', () => {
    const store = setup({ consent: granted });

    store.dispatch({
      type: 'config/updateSettings/fulfilled',
      meta: {
        arg: { aiAssistant: true, aiDataSharing: { [ADDRESS]: { ...granted, consent: 'denied' } } },
      },
    });

    expect(store.getState().workspace.settings.aiDataSharing?.[ADDRESS]?.consent).toBe('granted');
  });
});

describe('when the decision cannot be saved', () => {
  it('should not tell the service about it', async () => {
    vi.mocked(settingsPreload.updateAiSharingSettings).mockRejectedValueOnce(
      new Error('disk full'),
    );
    const store = setup({ consent: granted });

    await store.dispatch(actions.decide(false));

    expect(api.putConsent).not.toHaveBeenCalled();
    expect(aiSharing.purge).not.toHaveBeenCalled();
  });
});

describe('when the service keeps records for a wallet mismatch', () => {
  it('should back off instead of retrying every minute', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(
      ok({ accepted: 0, rejected: [{ recordId: 'a', reason: 'address_mismatch' }] }),
    );
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(backoffOf(store).failures).toBe(1);
    expect(backoffOf(store).nextUploadAt).toBeGreaterThan(Date.now());
  });

  it('should take an empty reply to an upload as everything accepted', async () => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek)
      .mockResolvedValueOnce([record('a')])
      .mockResolvedValueOnce([]);
    vi.mocked(api.uploadTurns).mockResolvedValue(ok(null) as never);
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.ack).toHaveBeenCalledWith(['a']);
  });
});

describe('when uploads for one wallet are backing off', () => {
  it('should not hold back another wallet that signs in', async () => {
    const OTHER = '0x000000000000000000000000000000000000beef';
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek).mockResolvedValue([record('a')]);
    vi.mocked(api.uploadTurns).mockResolvedValue(fail(503));
    const store = setup({ consent: granted });
    await store.dispatch(actions.flush());
    expect(backoffOf(store).failures).toBe(1);

    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(OTHER);
    store.dispatch({
      type: 'config/updateAiSharingSettings/fulfilled',
      meta: { arg: { aiDataSharing: { [ADDRESS]: granted, [OTHER]: granted } } },
    });
    vi.mocked(api.uploadTurns).mockClear();
    await store.dispatch(actions.flush());

    expect(api.uploadTurns).toHaveBeenCalledWith(OTHER, expect.anything());
    expect(backoffOf(store, OTHER).failures).toBe(1);
    expect(backoffOf(store).failures).toBe(1);
  });

  it("should not skip a wallet's flush because another wallet's is running", async () => {
    const OTHER = '0x000000000000000000000000000000000000beef';
    let finishFirst: (value: unknown) => void = () => {};
    vi.mocked(api.getConsent)
      .mockReturnValueOnce(new Promise(r => (finishFirst = r)) as never)
      .mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    const store = setup({ consent: granted });
    store.dispatch({
      type: 'config/updateAiSharingSettings/fulfilled',
      meta: { arg: { aiDataSharing: { [ADDRESS]: granted, [OTHER]: granted } } },
    });

    const first = store.dispatch(actions.flush());
    await settled();
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(OTHER);
    await store.dispatch(actions.flush());

    expect(api.getConsent).toHaveBeenCalledWith(OTHER);
    finishFirst(ok(serverSays(true, granted.decidedAt)));
    await first;
  });
});

describe('when deciding whether to read the queue again after an upload', () => {
  beforeEach(() => {
    vi.mocked(api.getConsent).mockResolvedValue(ok(serverSays(true, granted.decidedAt)));
    vi.mocked(aiSharing.peek)
      .mockResolvedValueOnce([record('a')])
      .mockResolvedValueOnce([]);
    vi.mocked(api.uploadTurns).mockResolvedValue(ok({ accepted: 1, rejected: [] }));
  });

  it('should not read it while no view shows it', async () => {
    const store = setup({ consent: granted });

    await store.dispatch(actions.flush());

    expect(aiSharing.list).not.toHaveBeenCalled();
  });

  it('should read it while Settings shows it', async () => {
    const store = setup({ consent: granted });
    store.dispatch(actions.showQueue());

    await store.dispatch(actions.flush());

    expect(aiSharing.list).toHaveBeenCalledWith(ADDRESS);
  });

  it('should stop reading it once the view closes', async () => {
    const store = setup({ consent: granted });
    store.dispatch(actions.showQueue());
    store.dispatch(actions.hideQueue());

    await store.dispatch(actions.flush());

    expect(aiSharing.list).not.toHaveBeenCalled();
  });
});
