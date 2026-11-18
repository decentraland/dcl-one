import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Emitter } from './emitter';
import { createEmitter } from './emitter';

type Events = { change: number; stop: void };

describe('createEmitter', () => {
  let events: Emitter<Events>;
  let calls: string[];

  beforeEach(() => {
    events = createEmitter<Events>();
    calls = [];
  });

  describe('when an event is emitted', () => {
    it('should call the handlers of its type in the order they were added', () => {
      events.on('change', value => calls.push(`first ${value}`));
      events.on('change', value => calls.push(`second ${value}`));
      events.on('stop', () => calls.push('stop'));

      events.emit('change', 7);

      expect(calls).toEqual(['first 7', 'second 7']);
    });

    it('should call the handlers of all types after them, with the type', () => {
      events.on('*', (type, value) => calls.push(`all ${String(type)} ${value}`));
      events.on('change', value => calls.push(`change ${value}`));

      events.emit('change', 7);
      events.emit('stop');

      expect(calls).toEqual(['change 7', 'all change 7', 'all stop undefined']);
    });

    it('should do nothing when the type has no handler', () => {
      expect(() => events.emit('change', 1)).not.toThrow();
    });
  });

  describe('when a handler is removed', () => {
    it('should not call it, and should call the other handlers', () => {
      const removed = vi.fn();
      const kept = vi.fn();
      events.on('change', removed);
      events.on('change', kept);

      events.off('change', removed);
      events.emit('change', 1);

      expect(removed).not.toHaveBeenCalled();
      expect(kept).toHaveBeenCalledWith(1);
    });

    it('should keep the other handlers when the handler was not added', () => {
      const kept = vi.fn();
      events.on('change', kept);

      events.off('change', vi.fn());
      events.emit('change', 1);

      expect(kept).toHaveBeenCalledTimes(1);
    });
  });

  describe('when a type is removed with no handler', () => {
    it('should remove all the handlers of that type only', () => {
      const change = vi.fn();
      const stop = vi.fn();
      events.on('change', change);
      events.on('change', change);
      events.on('stop', stop);

      events.off('change');
      events.emit('change', 1);
      events.emit('stop');

      expect(change).not.toHaveBeenCalled();
      expect(stop).toHaveBeenCalledTimes(1);
    });
  });

  describe('when a handler changes the handlers during an emit', () => {
    it('should finish the emit with the handlers it started with', () => {
      const late = vi.fn();
      const second = vi.fn();
      events.on('change', () => {
        events.off('change', second);
        events.on('change', late);
      });
      events.on('change', second);

      events.emit('change', 1);

      expect(second).toHaveBeenCalledTimes(1);
      expect(late).not.toHaveBeenCalled();
    });
  });

  describe('when all the handlers are cleared through the map', () => {
    it('should call no handler', () => {
      const change = vi.fn();
      events.on('change', change);

      events.all.clear();
      events.emit('change', 1);

      expect(change).not.toHaveBeenCalled();
    });
  });

  describe('when it gets a map of handlers', () => {
    it('should use that map', () => {
      const change = vi.fn();
      const withMap = createEmitter<Events>(new Map([['change', [change]]]));

      withMap.emit('change', 3);

      expect(change).toHaveBeenCalledWith(3);
    });
  });
});
