import { describe, expect, it, vi } from 'vitest';

import { onMicLevel, publishMicLevel } from './level';

function pcm(samples: number[]): Uint8Array {
  return new Uint8Array(new Int16Array(samples).buffer);
}

describe('when the microphone delivers a chunk', () => {
  it('should publish its peak as a level between 0 and 1', () => {
    const listener = vi.fn();
    const off = onMicLevel(listener);
    publishMicLevel(pcm([0, 8192, -16384, 100]));
    off();
    expect(listener).toHaveBeenCalledWith(0.5);
  });

  it('should stop telling a listener that unsubscribed', () => {
    const listener = vi.fn();
    onMicLevel(listener)();
    publishMicLevel(pcm([1000]));
    expect(listener).not.toHaveBeenCalled();
  });
});
