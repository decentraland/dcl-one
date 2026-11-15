import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';

import { REQUEST_TIMEOUT_MS, createController } from './controller';
import { PREVIEW_ORIGIN } from './protocol';

type Call = { namespace: 'scene' | 'emote'; method: string; params: unknown[]; result: unknown };

const CALLS: Call[] = [
  { namespace: 'scene', method: 'getScreenshot', params: [300, 200], result: 'data:image/png' },
  { namespace: 'scene', method: 'getMetrics', params: [], result: { triangles: 12 } },
  { namespace: 'emote', method: 'getLength', params: [], result: 2.5 },
  { namespace: 'emote', method: 'isPlaying', params: [], result: true },
  { namespace: 'emote', method: 'goTo', params: [1.25], result: undefined },
  { namespace: 'emote', method: 'play', params: [], result: undefined },
  { namespace: 'emote', method: 'pause', params: [], result: undefined },
];

describe('createController', () => {
  let frame: HTMLIFrameElement;
  let postMessage: Mock;

  beforeEach(() => {
    vi.useFakeTimers();
    frame = document.createElement('iframe');
    frame.id = 'preview';
    document.body.appendChild(frame);
    postMessage = vi
      .spyOn(frame.contentWindow!, 'postMessage')
      .mockImplementation(() => {}) as Mock;
  });

  afterEach(async () => {
    // Ends every request a test left open, so the module keeps no listener.
    await vi.advanceTimersByTimeAsync(REQUEST_TIMEOUT_MS);
    frame.remove();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  const invoke = ({ namespace, method, params }: Call): Promise<unknown> => {
    const controller = createController('preview') as unknown as Record<
      string,
      Record<string, (...args: unknown[]) => Promise<unknown>>
    >;
    const promise = controller[namespace][method](...params);
    promise.catch(() => {});
    return promise;
  };

  const lastRequest = () => postMessage.mock.calls.at(-1)![0].payload as { id: string };

  const respond = (payload: unknown, from: { origin?: string; source?: unknown } = {}) =>
    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'controller_response', payload },
        origin: from.origin ?? PREVIEW_ORIGIN,
        source: (from.source ?? frame.contentWindow) as MessageEventSource,
      }),
    );

  describe('when there is no frame with the id', () => {
    it('should throw', () => {
      expect(() => createController('missing')).toThrow(
        'Could not find an iframe with id="missing"',
      );
    });
  });

  describe.each(CALLS)('when $namespace.$method is called', call => {
    it('should post a request with its own id to the preview origin', () => {
      void invoke(call);
      void invoke(call);

      const [first, second] = postMessage.mock.calls;
      expect(first).toEqual([
        {
          type: 'controller_request',
          payload: {
            id: expect.stringMatching(/^preview-\d+$/),
            namespace: call.namespace,
            method: call.method,
            params: call.params,
          },
        },
        PREVIEW_ORIGIN,
      ]);
      expect(second[0].payload.id).not.toBe(first[0].payload.id);
    });

    it('should resolve with the result of the response that has its id', async () => {
      const promise = invoke(call);

      respond({ id: lastRequest().id, ok: true, result: call.result });

      await expect(promise).resolves.toEqual(call.result);
    });

    it('should reject with the error of the response that has its id', async () => {
      const promise = invoke(call);

      respond({ id: lastRequest().id, ok: false, error: 'a newer render has replaced it' });

      await expect(promise).rejects.toThrow('a newer render has replaced it');
    });

    it('should reject when the preview does not answer in time', async () => {
      const promise = invoke(call);
      const settled = vi.fn();
      promise.then(settled, settled);

      await vi.advanceTimersByTimeAsync(REQUEST_TIMEOUT_MS - 1);
      expect(settled).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(1);

      await expect(promise).rejects.toThrow(
        `The preview did not answer ${call.namespace}.${call.method}`,
      );
    });

    it('should ignore a response from another origin or another window', async () => {
      const promise = invoke(call);
      const settled = vi.fn();
      promise.then(settled, settled);
      const { id } = lastRequest();

      respond({ id, ok: true, result: 'forged' }, { origin: 'https://example.com' });
      respond({ id, ok: true, result: 'forged' }, { source: window });
      respond({ id: 'preview-unknown', ok: true, result: 'forged' });
      await vi.advanceTimersByTimeAsync(0);
      expect(settled).not.toHaveBeenCalled();

      respond({ id, ok: true, result: call.result });
      await expect(promise).resolves.toEqual(call.result);
    });
  });

  describe('when a response arrives after its request ended', () => {
    it('should ignore it', async () => {
      const promise = invoke(CALLS[0]);
      const { id } = lastRequest();
      respond({ id, ok: true, result: 'first' });

      respond({ id, ok: false, error: 'late' });

      await expect(promise).resolves.toBe('first');
    });
  });

  describe('when the frame is removed before a call', () => {
    it('should reject that call', async () => {
      const controller = createController('preview');
      frame.remove();

      await expect(controller.emote.play()).rejects.toThrow('Could not find an iframe');
    });
  });
});
