// The microphone's live loudness, for the waveform on the recording card. Kept out of the store:
// it changes ten times a second and only the window that records can show it.

type Listener = (level: number) => void;

const listeners = new Set<Listener>();

export function onMicLevel(listener: Listener): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

// Publishes the peak of a chunk of 16-bit little-endian PCM, as 0..1.
export function publishMicLevel(pcm: Uint8Array): void {
  if (listeners.size === 0) return;
  const view = new DataView(pcm.buffer, pcm.byteOffset, pcm.byteLength);
  let peak = 0;
  for (let i = 0; i + 1 < view.byteLength; i += 2) {
    const sample = Math.abs(view.getInt16(i, true));
    if (sample > peak) peak = sample;
  }
  const level = Math.min(1, peak / 0x8000);
  listeners.forEach(listener => listener(level));
}
