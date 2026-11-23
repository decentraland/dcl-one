import {
  RECORDED_EVENT_KINDS,
  type RecordedEvent,
  type RecordedEventKind,
} from '/shared/types/recording';

const MAX_TEXT = 200;
const MAX_NAME = 120;
const MAX_AREA = 60;

const KINDS: ReadonlySet<string> = new Set(RECORDED_EVENT_KINDS);

const MAX_VOCABULARY_NAME = 40;
const IDENTIFIER_LIKE = /^[\p{L}\p{N}_-]+(?: [\p{L}\p{N}_-]+)*$/u;

function isIdentifierLike(name: string): boolean {
  return name.length <= MAX_VOCABULARY_NAME && IDENTIFIER_LIKE.test(name);
}

/** One line of third-party text, cut to `max`, with nothing in it that can forge a line. */
export function cleanText(text: string, max: number): string {
  return (
    text
      // oxlint-disable-next-line no-control-regex
      .replace(/[\u0000-\u0008\u000e-\u001f\u007f-\u009f]+/g, '')
      .replace(/\s+/g, ' ')
      .trim()
      .slice(0, max)
  );
}

/** A title the user named, as one harmless line. */
export function cleanTitle(title: string): string {
  return cleanText(title, MAX_NAME);
}

/** An event as the inspector sent it, or null when it is not one the recorder accepts. */
export function sanitizeEvent(raw: unknown): RecordedEvent | null {
  if (!raw || typeof raw !== 'object') return null;
  const e = raw as Record<string, unknown>;
  if (typeof e.t !== 'number' || typeof e.text !== 'string') return null;
  if (typeof e.kind !== 'string' || !KINDS.has(e.kind)) return null;
  const event: RecordedEvent = {
    t: e.t,
    kind: e.kind as RecordedEventKind,
    text: cleanText(e.text, MAX_TEXT),
  };
  const label = typeof e.label === 'string' ? cleanText(e.label, MAX_TEXT) : '';
  if (label) event.label = label;
  const entity = e.entity as Record<string, unknown> | undefined;
  if (entity && typeof entity.id === 'number') {
    const name = typeof entity.name === 'string' ? cleanText(entity.name, MAX_NAME) : '';
    event.entity = { id: entity.id, ...(name && { name }) };
  }
  if (typeof e.area === 'string') event.area = cleanText(e.area, MAX_AREA);
  if (typeof e.x === 'number') event.x = e.x;
  if (typeof e.y === 'number') event.y = e.y;
  return event;
}

/** The identifier-like entity names the user worked with, as Whisper's initial prompt. */
export function vocabularyPrompt(events: RecordedEvent[]): string | undefined {
  const counts = new Map<string, number>();
  for (const e of events) {
    const name = e.entity?.name;
    if (name && isIdentifierLike(name)) counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  const names = [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([name]) => name);
  let prompt = '';
  for (const name of names) {
    const next = prompt ? `${prompt}, ${name}` : name;
    if (next.length > 200) break;
    prompt = next;
  }
  return prompt || undefined;
}
