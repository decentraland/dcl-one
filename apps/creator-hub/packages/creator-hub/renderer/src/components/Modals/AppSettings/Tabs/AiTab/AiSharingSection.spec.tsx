import { Provider } from 'react-redux';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  AI_SHARING_CONSENT_VERSION,
  type AiSharingConsent,
  type AiSharingQueuedItem,
} from '/shared/types/ai-sharing';

import { aiSharing } from '#preload';
import { AuthContext } from '/@/contexts/AuthContext';
import * as api from '/@/lib/aiSharingApi';
import { AuthServerProvider } from '/@/lib/auth';
import { FeatureFlag } from '/@/modules/store/featureFlags/types';
import { en } from '/@/modules/store/translation/locales';
import { flatten } from '/@/modules/store/translation/flatten';
import { setCurrentLocale } from '/@/modules/store/translation/utils';
import { createTestStore } from '../../../../../../tests/utils/testStore';
import { AiSharingSection } from './AiSharingSection';

vi.mock('/@/lib/aiSharingApi', () => ({
  isConfigured: vi.fn(() => true),
  getConsent: vi.fn(),
  putConsent: vi.fn(),
  uploadTurns: vi.fn(),
  listTurns: vi.fn(),
  listDigests: vi.fn(),
  eraseAll: vi.fn(),
  eraseTurn: vi.fn(),
  eraseConversation: vi.fn(),
  putRating: vi.fn(),
  MAX_RECORDS_PER_UPLOAD: 20,
  MAX_UPLOAD_BYTES: 900_000,
}));
vi.mock('/@/lib/auth', () => ({ AuthServerProvider: { getAccount: vi.fn() } }));

const copy = en.modal.app_settings.fields.ai_sharing;
const ADDRESS = '0x1234567890abcdef1234567890abcdef12345678';
const granted: AiSharingConsent = {
  consent: 'granted',
  version: AI_SHARING_CONSENT_VERSION,
  decidedAt: '2026-11-12T00:00:00Z',
  delivered: true,
};
const ok = <T,>(data: T) => ({ ok: true as const, status: 200, data });
const queuedItem = (n: number, prompt: string): AiSharingQueuedItem => ({
  recordId: `11111111-1111-4111-8111-11111111111${n}`,
  address: ADDRESS,
  startedAt: `2026-11-12T16:0${n}:00.000Z`,
  provider: 'claude',
  promptPreview: prompt,
});
const storedTurn = (n: number, prompt: string) =>
  ({
    recordId: `00000000-0000-4000-8000-00000000000${n}`,
    startedAt: `2026-11-1${n}T15:20:00.000Z`,
    prompt: { text: prompt, originalLength: prompt.length, truncated: false, redactions: {} },
  }) as api.AiSharingStoredTurn;

function mount({ signedIn = true, consent }: { signedIn?: boolean; consent?: AiSharingConsent }) {
  vi.mocked(AuthServerProvider.getAccount).mockReturnValue(signedIn ? ADDRESS : null);
  const store = createTestStore();
  store.dispatch({
    type: 'featureFlags/fetch/fulfilled',
    payload: { flags: { [FeatureFlag.AI_PROMPT_SHARING]: true }, variants: {} },
  });
  store.dispatch({
    type: 'config/updateAiSharingSettings/fulfilled',
    meta: { arg: { aiDataSharing: consent ? { [ADDRESS]: consent } : undefined } },
  });
  const signIn = vi.fn(async () => {});
  const auth = { wallet: signedIn ? ADDRESS : undefined, isSignedIn: signedIn, signIn } as never;
  render(
    <Provider store={store}>
      <AuthContext.Provider value={auth}>
        <AiSharingSection />
      </AuthContext.Provider>
    </Provider>,
  );
  return { store, signIn };
}

const sharingSwitch = () => screen.getByRole('switch', { name: copy.label });

beforeAll(() => {
  setCurrentLocale('en', flatten(en));
});

beforeEach(() => {
  vi.mocked(aiSharing.list).mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
});

describe('when signed out', () => {
  it('should disable the switch and offer to sign in', () => {
    const { signIn } = mount({ signedIn: false });

    expect(sharingSwitch()).toHaveProperty('disabled', true);
    expect(screen.getByText(copy.signin)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: copy.signin_link }));
    expect(signIn).toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: copy.delete })).toBeNull();
  });
});

describe('when signed in and not asked yet', () => {
  it('should show the switch off, and the show and delete row', () => {
    mount({});

    expect(sharingSwitch().getAttribute('aria-checked')).toBe('false');
    expect(screen.getByRole('button', { name: copy.viewer_show })).toBeTruthy();
    expect(screen.getByRole('button', { name: copy.delete })).toBeTruthy();
    expect(screen.queryByText(copy.queued_empty)).toBeNull();
  });
});

describe('when sharing is on with turns waiting', () => {
  it('should list what waits to upload', async () => {
    vi.mocked(aiSharing.list).mockResolvedValue([
      queuedItem(1, 'Make the fountain spray higher'),
      queuedItem(2, 'Turn the floor tiles into a checkerboard pattern'),
    ]);
    mount({ consent: granted });

    expect(sharingSwitch().getAttribute('aria-checked')).toBe('true');
    expect(await screen.findByText('Waiting to upload (2)')).toBeTruthy();
    expect(screen.getByText(/Make the fountain spray higher/)).toBeTruthy();
    expect(screen.getByText(/Turn the floor tiles into a checkerboard/)).toBeTruthy();
  });

  it('should keep the queue fresh only while it is on screen', async () => {
    const { store } = mount({ consent: granted });
    await waitFor(() => expect(store.getState().aiSharing.queueShown).toBe(1));

    cleanup();

    expect(store.getState().aiSharing.queueShown).toBe(0);
  });
});

describe('when looking at what was shared', () => {
  it('should list the shared turns, each with a delete button', async () => {
    vi.mocked(api.listTurns).mockResolvedValue(
      ok({
        records: [storedTurn(0, 'Add a door'), storedTurn(1, 'Make the lamp flicker')],
        next: null,
      }),
    );
    vi.mocked(api.eraseTurn).mockResolvedValue(ok({ deleted: 1 }));
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.viewer_show }));

    expect(await screen.findByText(/Make the lamp flicker/)).toBeTruthy();
    expect(screen.getByRole('button', { name: copy.viewer_hide })).toBeTruthy();
    const deletes = screen.getAllByRole('button', { name: copy.viewer_delete });
    expect(deletes).toHaveLength(2);
    fireEvent.click(deletes[0]);
    await waitFor(() => expect(screen.queryByText(/Add a door/)).toBeNull());
  });

  it('should say when the list could not be loaded', async () => {
    vi.mocked(api.listTurns).mockResolvedValue({ ok: false, status: 503, error: 'down' } as never);
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.viewer_show }));

    expect((await screen.findByRole('alert')).textContent).toBe(copy.viewer_failed);
  });
});

describe('when looking at the summaries', () => {
  it('should list each summary with its outcome and problems', async () => {
    vi.mocked(api.listDigests).mockResolvedValue(
      ok({
        digests: [
          {
            conversationId: 's2',
            goal: 'A lamp that glows at night',
            outcome: 'partial',
            frictions: [{ kind: 'wrong_sdk_knowledge' }],
            createdAt: '2026-11-12T09:00:00.000Z',
          },
        ],
        next: null,
      }),
    );
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.digests_show }));

    expect(await screen.findByText(/A lamp that glows at night · Partly achieved/)).toBeTruthy();
    expect(screen.getByText('Problems: wrong SDK knowledge')).toBeTruthy();
  });

  it('should leave the goal out of a summary whose notes were dropped', async () => {
    vi.mocked(api.listDigests).mockResolvedValue(
      ok({
        digests: [
          {
            conversationId: 's3',
            goal: null,
            outcome: 'partial',
            frictions: [{ kind: 'tool_failed' }],
            createdAt: '2026-11-12T09:00:00.000Z',
          },
        ],
        next: null,
      }),
    );
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.digests_show }));

    const line = await screen.findByText(/Partly achieved/);
    expect(line.textContent).not.toMatch(/·\s*·/);
  });
});

describe('when deleting everything', () => {
  it('should ask first, inline, and do nothing on cancel', () => {
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.delete }));

    expect(screen.getByText(copy.delete_confirm)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: copy.delete_cancel }));
    expect(screen.queryByText(copy.delete_confirm)).toBeNull();
    expect(api.eraseAll).not.toHaveBeenCalled();
  });

  it('should say how many turns were deleted, with sharing off', async () => {
    vi.mocked(api.eraseAll).mockResolvedValue(ok({ deleted: 12 }));
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.delete }));
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: copy.delete_confirm_yes }));
    });

    expect(await screen.findByText('Deleted 12 shared turns.')).toBeTruthy();
    expect(sharingSwitch().getAttribute('aria-checked')).toBe('false');
  });

  it('should say when the delete failed', async () => {
    vi.mocked(api.eraseAll).mockResolvedValue({ ok: false, status: 503, error: 'down' } as never);
    mount({ consent: granted });

    fireEvent.click(screen.getByRole('button', { name: copy.delete }));
    fireEvent.click(screen.getByRole('button', { name: copy.delete_confirm_yes }));

    expect((await screen.findByRole('alert')).textContent).toBe(copy.delete_failed);
  });
});
