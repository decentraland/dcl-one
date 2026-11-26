import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { playRecordingLimitChime } from './chime';

describe('playRecordingLimitChime', () => {
  let oscillators: { start: ReturnType<typeof vi.fn>; stop: ReturnType<typeof vi.fn> }[];
  let close: ReturnType<typeof vi.fn>;
  let FakeAudioContext: typeof AudioContext;

  beforeEach(() => {
    vi.useFakeTimers();
    oscillators = [];
    close = vi.fn(async () => {});
    const param = () => ({
      setValueAtTime: vi.fn(),
      linearRampToValueAtTime: vi.fn(),
      value: 0,
    });
    FakeAudioContext = class {
      currentTime = 0;
      destination = {};
      close = close;
      createOscillator() {
        const oscillator = {
          type: 'sine',
          frequency: param(),
          connect: vi.fn(),
          start: vi.fn(),
          stop: vi.fn(),
        };
        oscillators.push(oscillator);
        return oscillator;
      }
      createGain() {
        return { gain: param(), connect: vi.fn() };
      }
    } as unknown as typeof AudioContext;
  });

  afterEach(() => vi.useRealTimers());

  it('should play two tones and close the context afterwards', async () => {
    const done = playRecordingLimitChime(FakeAudioContext);
    expect(oscillators).toHaveLength(2);
    expect(oscillators.every(o => o.start.mock.calls.length === 1)).toBe(true);
    expect(close).not.toHaveBeenCalled();
    await vi.runAllTimersAsync();
    await done;
    expect(close).toHaveBeenCalledTimes(1);
  });
});
