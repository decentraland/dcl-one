import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { startMicrophone } from './microphone';

// Enough of the Web Audio surface for the capture setup: what each stage must be able to fail.
function fakeAudio({
  addModuleFails = false,
  workletNodeFails = false,
}: { addModuleFails?: boolean; workletNodeFails?: boolean } = {}) {
  const trackListeners = new Map<string, () => void>();
  const track = {
    stop: vi.fn(),
    label: 'USB mic',
    addEventListener: vi.fn((type: string, listener: () => void) =>
      trackListeners.set(type, listener),
    ),
    removeEventListener: vi.fn((type: string) => trackListeners.delete(type)),
  };
  const stream = { getTracks: () => [track], getAudioTracks: () => [track] };
  const close = vi.fn(async () => {});
  const source = { connect: vi.fn(), disconnect: vi.fn() };
  class FakeAudioContext {
    baseLatency = 0.01;
    audioWorklet = {
      addModule: vi.fn(async () => {
        if (addModuleFails) throw new Error('worklet failed');
      }),
    };
    createMediaStreamSource = vi.fn(() => source);
    close = close;
  }
  class FakeAudioWorkletNode {
    port = { onmessage: null as unknown };
    disconnect = vi.fn();
    constructor() {
      if (workletNodeFails) throw new Error('no such processor');
    }
  }
  vi.stubGlobal('navigator', { mediaDevices: { getUserMedia: vi.fn(async () => stream) } });
  vi.stubGlobal('AudioContext', FakeAudioContext);
  vi.stubGlobal('AudioWorkletNode', FakeAudioWorkletNode);
  vi.stubGlobal('URL', {
    createObjectURL: vi.fn(() => 'blob:worklet'),
    revokeObjectURL: vi.fn(),
  });
  vi.stubGlobal('Blob', class {});
  return { track, close, source, unplug: () => trackListeners.get('ended')?.() };
}

describe('startMicrophone', () => {
  afterEach(() => vi.unstubAllGlobals());

  describe('when the capture pipeline comes up', () => {
    let audio: ReturnType<typeof fakeAudio>;
    let microphone: Awaited<ReturnType<typeof startMicrophone>>;

    beforeEach(async () => {
      audio = fakeAudio();
      microphone = await startMicrophone(() => {});
    });

    it('should name the device and keep it open', () => {
      expect(microphone.label).toBe('USB mic');
      expect(audio.track.stop).not.toHaveBeenCalled();
    });

    it('should release the device and the context once on stop', () => {
      microphone.stop();
      microphone.stop();
      expect(audio.track.stop).toHaveBeenCalledTimes(1);
      expect(audio.close).toHaveBeenCalledTimes(1);
      expect(audio.source.disconnect).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the device goes away mid-recording', () => {
    it('should report that the input ended', async () => {
      const audio = fakeAudio();
      const ended = vi.fn();
      await startMicrophone(() => {}, ended);

      audio.unplug();

      expect(ended).toHaveBeenCalledTimes(1);
    });

    it('should not report it after the recording stopped on its own', async () => {
      const audio = fakeAudio();
      const ended = vi.fn();
      const microphone = await startMicrophone(() => {}, ended);

      microphone.stop();
      audio.unplug();

      expect(ended).not.toHaveBeenCalled();
    });
  });

  describe('when the worklet module fails to load', () => {
    it('should release the device before throwing', async () => {
      const audio = fakeAudio({ addModuleFails: true });
      await expect(startMicrophone(() => {})).rejects.toThrow('worklet failed');
      expect(audio.track.stop).toHaveBeenCalledTimes(1);
      expect(audio.close).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the worklet node cannot be created', () => {
    it('should release the device before throwing', async () => {
      const audio = fakeAudio({ workletNodeFails: true });
      await expect(startMicrophone(() => {})).rejects.toThrow('no such processor');
      expect(audio.track.stop).toHaveBeenCalledTimes(1);
      expect(audio.close).toHaveBeenCalledTimes(1);
    });
  });
});
