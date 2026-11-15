import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PreviewCamera, PreviewProjection } from '@dcl/schemas';
import type { WearableWithBlobs } from '@dcl/schemas';

import { PREVIEW_ORIGIN } from './protocol';
import { WearablePreview, buildPreviewUrl } from './WearablePreview';

const blob = { id: 'model.glb' } as WearableWithBlobs;

type TestWindow = {
  happyDOM: { settings: { fetch: { interceptor: unknown } } };
  Response: typeof Response;
};

describe('WearablePreview', () => {
  let pageRequests: number;

  beforeEach(() => {
    pageRequests = 0;
    // The frame must not load the hosted preview in a test.
    (window as unknown as TestWindow).happyDOM.settings.fetch.interceptor = {
      beforeAsyncRequest: async ({ window: frameWindow }: { window: TestWindow }) => {
        pageRequests++;
        return new frameWindow.Response('<html></html>');
      },
    };
  });

  afterEach(() => {
    cleanup();
    (window as unknown as TestWindow).happyDOM.settings.fetch.interceptor = null;
    vi.restoreAllMocks();
  });

  const setup = (props: Partial<Parameters<typeof WearablePreview>[0]> = {}) => {
    const onLoad = vi.fn();
    const onError = vi.fn();
    const onUpdate = vi.fn();
    const view = render(
      <WearablePreview
        id="preview"
        onLoad={onLoad}
        onError={onError}
        onUpdate={onUpdate}
        {...props}
      />,
    );
    const frame = view.container.querySelector('iframe')!;
    const postMessage = vi.spyOn(frame.contentWindow!, 'postMessage').mockImplementation(() => {});
    const receive = (
      data: unknown,
      from: { origin?: string; source?: unknown } = {},
    ): Promise<void> =>
      act(async () => {
        window.dispatchEvent(
          new MessageEvent('message', {
            data,
            origin: from.origin ?? PREVIEW_ORIGIN,
            source: (from.source ?? frame.contentWindow) as MessageEventSource,
          }),
        );
      });
    return { ...view, frame, postMessage, receive, onLoad, onError, onUpdate };
  };

  describe('when it renders', () => {
    it('should build the frame URL from the options', () => {
      const { frame } = setup({
        blob,
        profile: 'default',
        skin: '000000',
        wheelZoom: 2,
        disableBackground: true,
        disableAutoRotate: true,
        disableFace: true,
        disableDefaultWearables: true,
        projection: PreviewProjection.ORTHOGRAPHIC,
        camera: PreviewCamera.STATIC,
      });

      expect(frame.getAttribute('src')).toBe(
        'https://wearable-preview.decentraland.org?profile=default&skin=000000&camera=static' +
          '&projection=orthographic&wheelZoom=2&disableBackground=true&disableAutoRotate=true' +
          '&disableFace=true&disableDefaultWearables=true&unity=false',
      );
      expect(frame.id).toBe('preview');
      expect(frame.getAttribute('allow')).toBe('autoplay');
    });

    it('should leave out the options that have no value and encode the others', () => {
      expect(buildPreviewUrl({ profile: 'a b&c', skin: '', disableFace: false })).toBe(
        'https://wearable-preview.decentraland.org?profile=a%20b%26c&disableFace=false&unity=false',
      );
    });

    it('should not load the hosted page in this test', async () => {
      setup();
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 20));
      });
      expect(pageRequests).toBeGreaterThan(0);
    });

    it('should keep the frame URL when an option changes', () => {
      const { frame, rerender } = setup({ profile: 'default' });
      const url = frame.getAttribute('src');

      rerender(
        <WearablePreview
          id="preview"
          profile="other"
        />,
      );

      expect(frame.getAttribute('src')).toBe(url);
    });
  });

  describe('when the frame reports it is ready', () => {
    it('should send the model and the other options after that message, not before', async () => {
      const { postMessage, receive, onUpdate } = setup({ blob, disableBackground: true });
      expect(postMessage).not.toHaveBeenCalled();

      await receive({ type: 'ready' });

      expect(postMessage).toHaveBeenCalledTimes(1);
      expect(postMessage).toHaveBeenCalledWith(
        { type: 'update', payload: { options: { blob, disableBackground: true } } },
        PREVIEW_ORIGIN,
      );
      expect(onUpdate).toHaveBeenCalledTimes(1);
    });

    it('should send them one time when the frame repeats the message', async () => {
      const { postMessage, receive } = setup({ blob });

      await receive({ type: 'ready' });
      await receive({ type: 'ready' });

      expect(postMessage).toHaveBeenCalledTimes(1);
    });

    it('should send the options again when one changes', async () => {
      const { postMessage, receive, rerender } = setup({ blob });
      await receive({ type: 'ready' });
      const next = { id: 'other.glb' } as WearableWithBlobs;

      rerender(
        <WearablePreview
          id="preview"
          blob={next}
        />,
      );

      expect(postMessage).toHaveBeenCalledTimes(2);
      expect(postMessage).toHaveBeenLastCalledWith(
        { type: 'update', payload: { options: { blob: next } } },
        PREVIEW_ORIGIN,
      );
    });
  });

  describe('when the frame reports a load', () => {
    it('should call onLoad', async () => {
      const { receive, onLoad, onError } = setup();

      await receive({ type: 'load', payload: { renderer: 'babylon' } });

      expect(onLoad).toHaveBeenCalledTimes(1);
      expect(onError).not.toHaveBeenCalled();
    });
  });

  describe('when the frame reports an error', () => {
    it('should call onError with the message', async () => {
      const { receive, onLoad, onError } = setup();

      await receive({ type: 'error', payload: { message: 'could not load model' } });

      expect(onError).toHaveBeenCalledTimes(1);
      expect(onError.mock.calls[0][0]).toBeInstanceOf(Error);
      expect(onError.mock.calls[0][0].message).toBe('could not load model');
      expect(onLoad).not.toHaveBeenCalled();
    });
  });

  describe('when a message comes from another origin', () => {
    it('should ignore it', async () => {
      const { postMessage, receive, onLoad, onError, onUpdate } = setup({ blob });
      const from = { origin: 'https://example.com' };

      await receive({ type: 'ready' }, from);
      await receive({ type: 'load' }, from);
      await receive({ type: 'error', payload: { message: 'no' } }, from);

      expect(postMessage).not.toHaveBeenCalled();
      expect(onUpdate).not.toHaveBeenCalled();
      expect(onLoad).not.toHaveBeenCalled();
      expect(onError).not.toHaveBeenCalled();
    });
  });

  describe('when a message comes from another window', () => {
    it('should ignore it', async () => {
      const { postMessage, receive, onLoad, onError } = setup({ blob });
      const from = { source: window };

      await receive({ type: 'ready' }, from);
      await receive({ type: 'load' }, from);
      await receive({ type: 'error', payload: { message: 'no' } }, from);

      expect(postMessage).not.toHaveBeenCalled();
      expect(onLoad).not.toHaveBeenCalled();
      expect(onError).not.toHaveBeenCalled();
    });
  });
});
