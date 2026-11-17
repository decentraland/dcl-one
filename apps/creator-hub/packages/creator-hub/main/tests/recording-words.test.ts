import { describe, expect, it } from 'vitest';

import { toWords } from '../src/modules/recording/words';

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
