import { PreviewMessageType } from '@dcl/schemas';
import type { IEmoteController, ISceneController } from '@dcl/schemas';

import { PREVIEW_ORIGIN, postToPreview } from './protocol';

export type PreviewController = {
  scene: Pick<ISceneController, 'getScreenshot' | 'getMetrics'>;
  emote: Pick<IEmoteController, 'getLength' | 'isPlaying' | 'goTo' | 'play' | 'pause'>;
};

/** A frame that never loaded answers nothing, so every request ends after this time. */
export const REQUEST_TIMEOUT_MS = 30_000;

type PendingRequest = {
  source: Window;
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

const pending = new Map<string, PendingRequest>();
let nonce = 0;

function settle(id: string) {
  const request = pending.get(id);
  if (!request) return undefined;
  clearTimeout(request.timer);
  pending.delete(id);
  if (pending.size === 0) window.removeEventListener('message', handleResponse);
  return request;
}

function handleResponse(event: MessageEvent) {
  if (event.origin !== PREVIEW_ORIGIN) return;
  if (event.data?.type !== PreviewMessageType.CONTROLLER_RESPONSE) return;
  const payload = event.data.payload ?? {};
  if (pending.get(payload.id)?.source !== event.source) return;
  const request = settle(payload.id);
  if (!request) return;
  if (payload.ok) request.resolve(payload.result);
  else request.reject(new Error(payload.error));
}

function sendRequest<T>(
  id: string,
  namespace: keyof PreviewController,
  method: string,
  params: unknown[] = [],
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const frame = document.getElementById(id) as HTMLIFrameElement | null;
    const source = frame?.contentWindow;
    if (!frame || !source) {
      reject(new Error(`Could not find an iframe with id="${id}"`));
      return;
    }
    const requestId = `${id}-${nonce++}`;
    const timer = setTimeout(() => {
      settle(requestId);
      reject(new Error(`The preview did not answer ${namespace}.${method}`));
    }, REQUEST_TIMEOUT_MS);
    if (pending.size === 0) window.addEventListener('message', handleResponse);
    pending.set(requestId, {
      source,
      resolve: resolve as (result: unknown) => void,
      reject,
      timer,
    });
    postToPreview(frame, PreviewMessageType.CONTROLLER_REQUEST, {
      id: requestId,
      namespace,
      method,
      params,
    });
  });
}

/** Remote calls to the preview in the frame with this element id. */
export function createController(id: string): PreviewController {
  if (!document.getElementById(id)) {
    throw new Error(`Could not find an iframe with id="${id}"`);
  }
  return {
    scene: {
      getScreenshot: (width, height) => sendRequest(id, 'scene', 'getScreenshot', [width, height]),
      getMetrics: () => sendRequest(id, 'scene', 'getMetrics'),
    },
    emote: {
      getLength: () => sendRequest(id, 'emote', 'getLength'),
      isPlaying: () => sendRequest(id, 'emote', 'isPlaying'),
      goTo: seconds => sendRequest(id, 'emote', 'goTo', [seconds]),
      play: () => sendRequest(id, 'emote', 'play'),
      pause: () => sendRequest(id, 'emote', 'pause'),
    },
  };
}
