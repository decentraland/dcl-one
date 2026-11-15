const PREVIEW_URLS = {
  dev: 'https://wearable-preview.decentraland.zone',
  stg: 'https://wearable-preview.decentraland.today',
  prod: 'https://wearable-preview.decentraland.org',
} as const;

type PreviewEnv = keyof typeof PREVIEW_URLS;

function isPreviewEnv(value: string | undefined): value is PreviewEnv {
  return !!value && Object.hasOwn(PREVIEW_URLS, value);
}

/**
 * The hosted preview for the page that shows it: the page's `env` query parameter chooses first,
 * then the top-level domain of its host; any other page gets production.
 */
export function resolvePreviewUrl({ search, host }: Pick<Location, 'search' | 'host'>): string {
  const params = new URLSearchParams(search);
  const env = (params.get('ENV') || params.get('env'))?.toLowerCase();
  if (isPreviewEnv(env)) return PREVIEW_URLS[env];
  if (host.endsWith('.today') || host.endsWith('.net')) return PREVIEW_URLS.stg;
  if (host.endsWith('.io') || host.endsWith('.zone')) return PREVIEW_URLS.dev;
  return PREVIEW_URLS.prod;
}

/** The hosted avatar preview that the frame loads. Messages are trusted from this origin only. */
export const PREVIEW_URL = resolvePreviewUrl(window.location);
export const PREVIEW_ORIGIN = new URL(PREVIEW_URL).origin;

/** True for a message that the hosted preview inside `frame` sent. */
export function isPreviewMessage(
  event: MessageEvent,
  frame: HTMLIFrameElement | null | undefined,
): boolean {
  const source = frame?.contentWindow;
  return !!source && event.source === source && event.origin === PREVIEW_ORIGIN;
}

export function postToPreview(frame: HTMLIFrameElement, type: string, payload: unknown) {
  frame.contentWindow?.postMessage({ type, payload }, PREVIEW_ORIGIN);
}
