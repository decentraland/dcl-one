import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import * as api from '/@/lib/creatorFundApi';
import { actions } from '/@/modules/store/creatorFund';

import { createTestStore } from '../../../../tests/utils/testStore';
import { useResendHeld } from './emailCode';
import { ok } from './fixtures';

vi.mock('/@/lib/creatorFundApi', () => ({ postEmailVerification: vi.fn() }));
vi.mock('#store', async () => {
  const { useDispatch, useSelector } = await import('react-redux');
  return { useDispatch, useSelector };
});

describe('useResendHeld', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  describe('when the last code was just sent', () => {
    it('should let a resend go once the hold ends, also when its timer fires a millisecond early', async () => {
      const store = createTestStore();
      vi.mocked(api.postEmailVerification).mockResolvedValue(
        ok(
          {
            sent_at: new Date().toISOString(),
            expires_at: new Date(Date.now() + 600e3).toISOString(),
            resend_available_at: new Date(Date.now() + 600).toISOString(),
          },
          202,
        ) as never,
      );
      await store.dispatch(actions.sendEmailVerification());
      const { result } = renderHook(() => useResendHeld(), {
        wrapper: ({ children }: { children: ReactNode }) => (
          <Provider store={store}>{children}</Provider>
        ),
      });
      expect(result.current).toBe(true);

      // The clock reads a millisecond behind the timers, as Node's often does.
      vi.setSystemTime(Date.now() - 1);
      act(() => vi.advanceTimersByTime(600));
      expect(result.current).toBe(true);
      act(() => vi.advanceTimersByTime(1));

      expect(result.current).toBe(false);
    });
  });
});
