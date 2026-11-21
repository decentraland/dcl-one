// The sound played when a recording stops on its own at the length limit: two short descending
// tones, synthesized so no audio asset ships for it. The user is likely looking at the scene,
// not the button, so they hear that the microphone is off before they see it.

const TONES: { hz: number; at: number }[] = [
  { hz: 880, at: 0 },
  { hz: 587, at: 0.16 },
];
const TONE_SECONDS = 0.14;
const GAIN = 0.12;

export async function playRecordingLimitChime(
  AudioContextImpl: typeof AudioContext = AudioContext,
): Promise<void> {
  const context = new AudioContextImpl();
  try {
    const start = context.currentTime + 0.01;
    for (const { hz, at } of TONES) {
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = 'sine';
      oscillator.frequency.value = hz;
      gain.gain.setValueAtTime(0, start + at);
      gain.gain.linearRampToValueAtTime(GAIN, start + at + 0.01);
      gain.gain.linearRampToValueAtTime(0, start + at + TONE_SECONDS);
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.start(start + at);
      oscillator.stop(start + at + TONE_SECONDS);
    }
    const last = TONES[TONES.length - 1];
    await new Promise(resolve => setTimeout(resolve, (last.at + TONE_SECONDS + 0.05) * 1000));
  } finally {
    await context.close().catch(() => undefined);
  }
}
