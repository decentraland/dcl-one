import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { actions } from '/@/modules/store/creatorFund';
import { createTestStore } from '../../../../tests/utils/testStore';
import { useResendHeld } from './emailCode';

vi.mock('#store', async () => {
  const { useDispatch, useSelector } = await import('react-redux');
  return { useDispatch, useSelector };
});

describe('when a resend is held until the service deadline', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('should keep waiting, then release the hold at the deadline without another store update', () => {
    vi.useFakeTimers();
    const now = Date.parse('2026-11-01T12:00:00Z');
    const clock = vi.spyOn(Date, 'now').mockReturnValue(now);
    const store = createTestStore();
    store.dispatch(actions.sendEmailVerification.pending('verification', undefined));
    store.dispatch(
      actions.sendEmailVerification.fulfilled(
        {
          sent_at: new Date(now).toISOString(),
          expires_at: new Date(now + 600_000).toISOString(),
          resend_available_at: new Date(now + 60_000).toISOString(),
        },
        'verification',
        undefined,
      ),
    );
    const { result } = renderHook(() => useResendHeld(), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <Provider store={store}>{children}</Provider>
      ),
    });
    expect(result.current).toBe(true);

    clock.mockReturnValue(now + 59_999);
    act(() => vi.advanceTimersByTime(60_000));
    expect(result.current).toBe(true);

    clock.mockReturnValue(now + 60_000);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toBe(false);
  });

  it('should wait out a long hold without repeatedly waking or releasing it early', () => {
    vi.useFakeTimers();
    const now = Date.parse('2026-11-01T12:00:00Z');
    vi.setSystemTime(now);
    const maxTimerDelay = 2_147_483_647;
    const resendAt = now + maxTimerDelay + 60_000;
    const store = createTestStore();
    store.dispatch(actions.sendEmailVerification.pending('verification', undefined));
    store.dispatch(
      actions.sendEmailVerification.fulfilled(
        {
          sent_at: new Date(now).toISOString(),
          expires_at: new Date(resendAt + 600_000).toISOString(),
          resend_available_at: new Date(resendAt).toISOString(),
        },
        'verification',
        undefined,
      ),
    );
    const render = vi.fn(() => useResendHeld());
    const { result } = renderHook(render, {
      wrapper: ({ children }: { children: ReactNode }) => (
        <Provider store={store}>{children}</Provider>
      ),
    });
    expect(result.current).toBe(true);
    const initialRenders = render.mock.calls.length;

    act(() => vi.advanceTimersByTime(1_000));
    expect(result.current).toBe(true);
    expect(render).toHaveBeenCalledTimes(initialRenders);

    act(() => vi.advanceTimersByTime(maxTimerDelay - 1_000));
    expect(result.current).toBe(true);
    act(() => vi.advanceTimersByTime(59_999));
    expect(result.current).toBe(true);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toBe(false);
  });
});
