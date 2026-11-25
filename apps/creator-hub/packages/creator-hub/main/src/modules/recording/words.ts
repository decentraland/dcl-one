import type { TranscriptWord } from '/shared/types/recording';

import type { SpeechRegion } from './vad';

/** One stretch of speech in the recording (ms), numbered in the order it was found. */
export type Region = SpeechRegion & { segment: number };

const WHISPER_PADS_EVERY_CLIP_TO_MS = 30_000;
const MERGE_GAP_MS = 2000;

/** The stretches of speech as the padded clips Whisper is given, numbered in order. */
export function toWindows(
  regions: SpeechRegion[],
  maxWindowMs = WHISPER_PADS_EVERY_CLIP_TO_MS,
): Region[] {
  const windows: Region[] = [];
  for (const region of regions) {
    const last = windows[windows.length - 1];
    if (last && region.start - last.end <= MERGE_GAP_MS && region.end - last.start <= maxWindowMs) {
      last.end = region.end;
      continue;
    }
    windows.push({ start: region.start, end: region.end, segment: windows.length });
  }
  return windows;
}

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

/** The transcription ran out of time: `words` holds what was transcribed before that. */
export class TranscriptCutShort extends Error {
  constructor(readonly words: TranscriptWord[]) {
    super('Transcription stopped at the time limit.');
    this.name = 'TranscriptCutShort';
  }
}
