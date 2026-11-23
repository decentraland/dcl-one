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

describe('when the resend timer fires before the service deadline', () => {
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
});
