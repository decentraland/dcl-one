export type EntityRef = { id: number; name?: string };

/** Every {@link RecordedEvent} kind. Creator Hub mirrors this list in `shared/types/recording.ts`. */
export const RECORDED_EVENT_KINDS = [
  'click',
  'dblclick',
  'contextmenu',
  'component',
  'dialog',
  'entity',
  'hover',
  'input',
  'key',
  'menu',
  'mode',
  'panel',
  'player',
  'scene-click',
  'scene-appear',
  'scene-remove',
  'scroll',
  'select',
  'setting',
  'viewport-hover',
] as const;

export type RecordedEventKind = (typeof RECORDED_EVENT_KINDS)[number];

/** One thing the user did while the host was recording a session. */
export type RecordedEvent = {
  /** `Date.now()` when it happened (epoch ms), so it lines up with the host's clock. */
  t: number;
  kind: RecordedEventKind;
  /** One human-readable line, at most {@link MAX_TEXT_LENGTH} characters. */
  text: string;
  /** What a `scene-appear` or `scene-remove` event is about, without the event's own wording. */
  label?: string;
  entity?: EntityRef;
  area?: string;
  /** Pointer position in CSS px of the inspector document, for pointer events. */
  x?: number;
  y?: number;
};

export type RecordedEventInput = Omit<RecordedEvent, 't'> & { t?: number };

export type Emit = (event: RecordedEventInput) => void;

/** Stops a tracker: detaches everything it attached and emits whatever it was still holding. */
export type StopTracker = () => void;

export const MAX_TEXT_LENGTH = 200;
