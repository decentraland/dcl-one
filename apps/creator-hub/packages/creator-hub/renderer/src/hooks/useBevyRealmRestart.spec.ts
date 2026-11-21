import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { BevyRealmSupervisionEvent } from '/shared/types/ipc';

const { editor } = await import('#preload');
const { useBevyRealmRestart } = await import('./useBevyRealmRestart');

describe('useBevyRealmRestart', () => {
  let emit: (event: BevyRealmSupervisionEvent) => void;
  let unsubscribe: ReturnType<typeof vi.fn>;
  let handlers: {
    onStopped: ReturnType<typeof vi.fn>;
    onRestarting: ReturnType<typeof vi.fn>;
    onRestarted: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    unsubscribe = vi.fn();
    vi.mocked(editor.onBevyRealmSupervision).mockImplementation(callback => {
      emit = callback;
      return unsubscribe;
    });
    handlers = { onStopped: vi.fn(), onRestarting: vi.fn(), onRestarted: vi.fn() };
  });

  describe('when the Bevy renderer is active for a project', () => {
    beforeEach(() => {
      renderHook(() => useBevyRealmRestart('/scenes/demo', handlers));
    });

    describe('and main reports that its Multiplayer Server stopped', () => {
      let event: BevyRealmSupervisionEvent;

      beforeEach(() => {
        event = {
          kind: 'stopped',
          path: '/scenes/demo',
          cause: 'server',
          message: 'Multiplayer Server exited with code 1',
          restartInMs: 1_000,
        };
        emit(event);
      });

      it('should hand the event to onStopped', () => {
        expect(handlers.onStopped).toHaveBeenCalledWith(event);
      });
    });

    describe('and main starts replacing the realm', () => {
      beforeEach(() => {
        emit({ kind: 'restarting', path: '/scenes/demo' });
      });

      it('should call onRestarting', () => {
        expect(handlers.onRestarting).toHaveBeenCalledTimes(1);
      });
    });

    describe('and the new realm serves', () => {
      beforeEach(() => {
        emit({
          kind: 'restarted',
          path: '/scenes/demo',
          url: 'http://localhost:8005',
          wsUrl: 'ws://localhost:8005/data-layer',
        });
      });

      it('should hand its URLs to onRestarted', () => {
        expect(handlers.onRestarted).toHaveBeenCalledWith({
          url: 'http://localhost:8005',
          wsUrl: 'ws://localhost:8005/data-layer',
        });
      });
    });

    describe('and another project’s realm is restarted', () => {
      beforeEach(() => {
        emit({ kind: 'restarting', path: '/scenes/other' });
      });

      it('should ignore it', () => {
        expect(handlers.onRestarting).not.toHaveBeenCalled();
      });
    });
  });

  describe('when the hook unmounts', () => {
    beforeEach(() => {
      const { unmount } = renderHook(() => useBevyRealmRestart('/scenes/demo', handlers));
      unmount();
    });

    it('should unsubscribe', () => {
      expect(unsubscribe).toHaveBeenCalled();
    });
  });

  describe('when the Bevy renderer is not active', () => {
    beforeEach(() => {
      renderHook(() => useBevyRealmRestart(undefined, handlers));
    });

    it('should not subscribe', () => {
      expect(editor.onBevyRealmSupervision).not.toHaveBeenCalled();
    });
  });
});
