import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Notification } from '/@/modules/store/snackbar/types';

import { Snackbar } from '.';

const dismissed = vi.fn();
const notifications: Notification[] = [];

vi.mock('/@/hooks/useSnackbar', () => ({
  useSnackbar: () => ({
    notifications,
    close: () => vi.fn(),
    dismiss: (id: string) => (reason: string) => dismissed(id, reason),
  }),
}));

vi.mock('/@/hooks/useEditor', () => ({ useEditor: () => ({ isInstallingProject: false }) }));

vi.mock('/@/hooks/useExternalAgentAccess', () => ({ useExternalAgentAccess: vi.fn() }));

const wait = (ms: number) =>
  act(() => {
    vi.advanceTimersByTime(ms);
  });

describe('Snackbar', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    dismissed.mockClear();
    notifications.length = 0;
    notifications.push({
      id: 'saved',
      type: 'generic',
      severity: 'success',
      message: 'Scene saved',
      duration: 4_000,
    });
    render(<Snackbar />);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('when a notice reaches its duration', () => {
    it('should ask to close with the timeout reason', () => {
      wait(3_900);
      expect(dismissed).not.toHaveBeenCalled();

      wait(200);

      expect(dismissed).toHaveBeenCalledTimes(1);
      expect(dismissed).toHaveBeenCalledWith('saved', 'timeout');
    });
  });

  describe('when the window is not the active one', () => {
    beforeEach(() => {
      wait(3_000);
      fireEvent.blur(window);
      wait(60_000);
    });

    it('should keep the notice', () => {
      expect(dismissed).not.toHaveBeenCalled();
      expect(screen.getByText('Scene saved')).toBeTruthy();
    });

    describe('and the window is active again', () => {
      beforeEach(() => {
        fireEvent.focus(window);
      });

      it('should ask to close after half of the duration', () => {
        wait(1_900);
        expect(dismissed).not.toHaveBeenCalled();

        wait(200);

        expect(dismissed).toHaveBeenCalledTimes(1);
        expect(dismissed).toHaveBeenCalledWith('saved', 'timeout');
      });
    });
  });

  describe('when the pointer leaves a notice with little time left', () => {
    beforeEach(() => {
      wait(3_900);
      fireEvent.mouseEnter(screen.getByText('Scene saved'));
      wait(60_000);
      fireEvent.mouseLeave(screen.getByText('Scene saved'));
    });

    it('should ask to close after half of the duration', () => {
      wait(1_900);
      expect(dismissed).not.toHaveBeenCalled();

      wait(200);

      expect(dismissed).toHaveBeenCalledTimes(1);
      expect(dismissed).toHaveBeenCalledWith('saved', 'timeout');
    });
  });
});
