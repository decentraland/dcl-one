import { describe, expect, it } from 'vitest';

import { toWindows, toWords } from '../src/modules/recording/words';

describe('when turning a region of Whisper segments into transcript words', () => {
  const region = { start: 5000, end: 6000, segment: 2 };

  it('should move each word onto the recording clock and tag its stretch of speech', () => {
    expect(
      toWords(region, [
        { t0: 0, t1: 200, text: ' delete' },
        { t0: 200, t1: 450, text: ' this' },
      ]),
    ).toEqual([
      { start: 5000, end: 5200, text: ' delete', segment: 2 },
      { start: 5200, end: 5450, text: ' this', segment: 2 },
    ]);
  });

  it('should never let a word end after the region it was transcribed in', () => {
    expect(toWords(region, [{ t0: 900, t1: 1400, text: ' one' }])).toEqual([
      { start: 5900, end: 6000, text: ' one', segment: 2 },
    ]);
  });

  it('should return nothing for a region Whisper read nothing in', () => {
    expect(toWords(region, [])).toEqual([]);
  });
});

describe('when grouping the stretches of speech into transcription windows', () => {
  it('should put a breath-long pause in one window, since whisper pads every clip to 30 s', () => {
    expect(
      toWindows([
        { start: 1000, end: 3000 },
        { start: 3600, end: 5000 },
      ]),
    ).toEqual([{ start: 1000, end: 5000, segment: 0 }]);
  });

  it('should start a new window when the user was quiet for longer', () => {
    expect(
      toWindows([
        { start: 1000, end: 3000 },
        { start: 9000, end: 11_000 },
      ]),
    ).toEqual([
      { start: 1000, end: 3000, segment: 0 },
      { start: 9000, end: 11_000, segment: 1 },
    ]);
  });

  it('should start a new window rather than let one pass 30 seconds', () => {
    const regions = Array.from({ length: 6 }, (_, i) => ({
      start: i * 6000,
      end: i * 6000 + 5000,
    }));
    const windows = toWindows(regions);

    expect(windows.map(w => [w.start, w.end])).toEqual([
      [0, 29_000],
      [30_000, 35_000],
    ]);
    expect(windows.map(w => w.segment)).toEqual([0, 1]);
  });

  it('should leave an empty recording empty', () => {
    expect(toWindows([])).toEqual([]);
  });
});
