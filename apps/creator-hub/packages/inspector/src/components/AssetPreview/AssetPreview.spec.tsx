import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';

import { AssetPreview } from './AssetPreview';

const preview = vi.hoisted(() => ({
  props: null as {
    onLoad?: () => void;
    onError?: (error: Error) => void;
    onUpdate?: () => void;
  } | null,
  getScreenshot: vi.fn(),
  getMetrics: vi.fn(),
}));

vi.mock('../WearablePreview', () => ({
  WearablePreview: Object.assign(
    (props: NonNullable<typeof preview.props>) => {
      preview.props = props;
      return <iframe />;
    },
    {
      createController: () => ({
        scene: { getScreenshot: preview.getScreenshot, getMetrics: preview.getMetrics },
      }),
    },
  ),
}));

describe('AssetPreview', () => {
  let onScreenshot: Mock;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(console, 'error').mockImplementation(() => {});
    preview.props = null;
    preview.getScreenshot.mockReset();
    preview.getMetrics.mockReset().mockResolvedValue({ triangles: 12 });
    onScreenshot = vi.fn();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  const renderModel = () =>
    render(
      <AssetPreview
        value={new File([], 'model.glb')}
        onScreenshot={onScreenshot}
      />,
    );

  /** The preview component reporting that it sent the model to the frame. */
  const modelSent = () => preview.props!.onUpdate!();

  describe('when the preview renders the model and returns a screenshot', () => {
    it('should report the thumbnail it captured', async () => {
      preview.getScreenshot.mockResolvedValue('data:image/png;base64,thumbnail');
      renderModel();
      modelSent();

      await act(async () => {
        preview.props!.onLoad!();
        await vi.advanceTimersByTimeAsync(1000);
      });

      expect(onScreenshot).toHaveBeenCalledTimes(1);
      expect(onScreenshot).toHaveBeenCalledWith('data:image/png;base64,thumbnail');
    });
  });

  describe('when the preview reports it could not render the model', () => {
    it('should report no thumbnail so the import can go ahead without one', async () => {
      renderModel();

      await act(async () => {
        preview.props!.onError!(new Error('could not load model'));
      });

      expect(onScreenshot).toHaveBeenCalledTimes(1);
      expect(onScreenshot).toHaveBeenCalledWith(undefined);
    });
  });

  describe('when taking the screenshot fails', () => {
    it('should report no thumbnail rather than leave the promise unhandled', async () => {
      preview.getScreenshot.mockRejectedValue(new Error('no renderer'));
      renderModel();
      modelSent();

      await act(async () => {
        preview.props!.onLoad!();
      });

      expect(onScreenshot).toHaveBeenCalledTimes(1);
      expect(onScreenshot).toHaveBeenCalledWith(undefined);
    });
  });

  describe('when the preview never responds, as a frame the browser refused to load (#1485)', () => {
    it('should report no thumbnail once it gives up, since nothing else can end the wait', async () => {
      renderModel();

      expect(onScreenshot).not.toHaveBeenCalled();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(30_000);
      });

      expect(onScreenshot).toHaveBeenCalledTimes(1);
      expect(onScreenshot).toHaveBeenCalledWith(undefined);
      expect(console.error).toHaveBeenCalled();
    });
  });

  describe('when the frame loads before the model has been sent', () => {
    it('should not screenshot that empty scene', async () => {
      preview.getScreenshot.mockResolvedValue('data:image/png;base64,empty');
      renderModel();

      await act(async () => {
        preview.props!.onLoad!();
        await vi.advanceTimersByTimeAsync(1000);
      });

      expect(preview.getScreenshot).not.toHaveBeenCalled();
      expect(onScreenshot).not.toHaveBeenCalled();
    });
  });

  describe('when a newer render replaces the scene being screenshotted', () => {
    it('should wait for the next load and report its thumbnail', async () => {
      preview.getScreenshot
        .mockRejectedValueOnce(new Error('The scene was disposed, a newer render has replaced it'))
        .mockResolvedValueOnce('data:image/png;base64,model');
      renderModel();
      modelSent();

      await act(async () => {
        preview.props!.onLoad!();
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(onScreenshot).not.toHaveBeenCalled();

      await act(async () => {
        preview.props!.onLoad!();
        await vi.advanceTimersByTimeAsync(1000);
      });

      expect(onScreenshot).toHaveBeenCalledTimes(1);
      expect(onScreenshot).toHaveBeenCalledWith('data:image/png;base64,model');
    });
  });

  describe('when the frame reports its empty scene loaded before the model', () => {
    it('should skip that render and screenshot the model once it loads', async () => {
      preview.getMetrics
        .mockResolvedValueOnce({ triangles: 0 })
        .mockResolvedValueOnce({ triangles: 12 });
      preview.getScreenshot.mockResolvedValue('data:image/png;base64,model');
      renderModel();
      modelSent();

      await act(async () => {
        preview.props!.onLoad!();
        await vi.advanceTimersByTimeAsync(1000);
      });
      expect(preview.getScreenshot).not.toHaveBeenCalled();

      await act(async () => {
        preview.props!.onLoad!();
        await vi.advanceTimersByTimeAsync(1000);
      });

      expect(onScreenshot).toHaveBeenCalledTimes(1);
      expect(onScreenshot).toHaveBeenCalledWith('data:image/png;base64,model');
    });
  });
});
