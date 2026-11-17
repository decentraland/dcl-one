import type { TranscriptWord } from '/shared/types/recording';

import type { SpeechRegion } from './vad';

/** One stretch of speech in the recording (ms), numbered in the order it was found. */
export type Region = SpeechRegion & { segment: number };

/** What Whisper returns per word with `maxLen: 1`: ms from the start of the clip it was given. */
export type WhisperSegment = { t0: number; t1: number; text: string };

/** Whisper's words for one region, placed on the recording's clock. */
export function toWords(region: Region, segments: WhisperSegment[]): TranscriptWord[] {
  return segments.map(segment => ({
    start: region.start + segment.t0,
    end: Math.min(region.end, region.start + segment.t1),
    text: segment.text,
    segment: region.segment,
  }));
}
