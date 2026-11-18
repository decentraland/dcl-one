import { afterEach, describe, expect, it, vi } from 'vitest';

import { logger, redactSecret } from './logger';
import { store } from '.';

describe('redactSecret', () => {
  describe('when an action carries a password in its thunk argument', () => {
    it('should hide the password and keep the rest', () => {
      const action = {
        type: 'management/updateWorldPermissions/pending',
        meta: { arg: { worldName: 'plaza.dcl.eth', options: { secret: 'abcdefg12' } } },
      };

      expect(redactSecret(action)).toEqual({
        type: 'management/updateWorldPermissions/pending',
        meta: { arg: { worldName: 'plaza.dcl.eth', options: { secret: '<redacted>' } } },
      });
      expect(action.meta.arg.options.secret).toBe('abcdefg12');
    });
  });

  describe('when an action carries no password', () => {
    it('should pass it through untouched', () => {
      const action = { type: 'management/fetchWorldPermissions/pending', meta: { arg: {} } };

      expect(redactSecret(action)).toBe(action);
    });
  });
});

describe('the app store', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('when an action carrying a password is dispatched', () => {
    it('should log it without the password', () => {
      const methods = ['log', 'info', 'group', 'groupCollapsed', 'groupEnd'] as const;
      const spies = methods.map(method => vi.spyOn(console, method).mockImplementation(() => {}));

      store.dispatch({
        type: 'test/secretAction',
        meta: { arg: { options: { secret: 'abcdefg12' } } },
      });

      const logged = JSON.stringify(spies.flatMap(spy => spy.mock.calls));
      expect(logged).toContain('<redacted>');
      expect(logged).not.toContain('abcdefg12');
    });
  });
});

describe('logger', () => {
  const api = (states: unknown[]) => ({ dispatch: vi.fn(), getState: () => states.shift() });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('when an action goes through', () => {
    it('should print the state before it, the action and the state after it in one group', () => {
      const group = vi.spyOn(console, 'group').mockImplementation(() => {});
      const log = vi.spyOn(console, 'log').mockImplementation(() => {});
      const groupEnd = vi.spyOn(console, 'groupEnd').mockImplementation(() => {});
      const action = { type: 'test/action' };
      const next = vi.fn(() => 'result');

      const result = logger(api([{ count: 1 }, { count: 2 }]))(next)(action);

      expect(result).toBe('result');
      expect(next).toHaveBeenCalledWith(action);
      expect(group.mock.calls[0][0]).toMatch(
        /^%c action %ctest\/action %c@ \d{2}:\d{2}:\d{2}\.\d{3}$/,
      );
      expect(log.mock.calls.map(call => [call[0], call[2]])).toEqual([
        ['%c prev state', { count: 1 }],
        ['%c action    ', action],
        ['%c next state', { count: 2 }],
      ]);
      expect(groupEnd).toHaveBeenCalledTimes(1);
    });
  });

  describe('when a reducer throws', () => {
    it('should print the error and throw it again', () => {
      vi.spyOn(console, 'group').mockImplementation(() => {});
      vi.spyOn(console, 'groupEnd').mockImplementation(() => {});
      const log = vi.spyOn(console, 'log').mockImplementation(() => {});
      const error = new Error('reducer failed');
      const next = vi.fn(() => {
        throw error;
      });

      expect(() => logger(api([{}, {}]))(next)({ type: 'test/action' })).toThrow(error);
      expect(log.mock.calls.map(call => [call[0], call[2]])).toContainEqual([
        '%c error     ',
        error,
      ]);
    });
  });
});
