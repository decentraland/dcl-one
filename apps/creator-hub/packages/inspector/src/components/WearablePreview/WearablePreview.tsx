import { useEffect, useMemo, useRef, useState } from 'react';
import { PreviewMessageType } from '@dcl/schemas';
import type {
  EmoteWithBlobs,
  PreviewCamera,
  PreviewEmote,
  PreviewProjection,
  WearableWithBlobs,
} from '@dcl/schemas';

import { createController } from './controller';
import { PREVIEW_URL, isPreviewMessage, postToPreview } from './protocol';

import styles from './WearablePreview.module.css';

export type PreviewOptions = {
  /** A local model. It cannot go in the URL, so the frame gets it in a message. */
  blob?: WearableWithBlobs | EmoteWithBlobs;
  profile?: string;
  skin?: string;
  emote?: PreviewEmote;
  camera?: PreviewCamera;
  projection?: PreviewProjection;
  wheelZoom?: number;
  disableBackground?: boolean;
  disableAutoRotate?: boolean;
  disableFace?: boolean;
  disableDefaultWearables?: boolean;
};

export type Props = PreviewOptions & {
  /** The frame's element id, which `createController` takes. */
  id: string;
  onLoad?: () => void;
  onError?: (error: Error) => void;
  /** Called each time the options are sent to the frame. */
  onUpdate?: () => void;
};

const URL_PARAMS = [
  'profile',
  'skin',
  'emote',
  'camera',
  'projection',
  'wheelZoom',
  'disableBackground',
  'disableAutoRotate',
  'disableFace',
  'disableDefaultWearables',
] as const;

export function buildPreviewUrl(options: PreviewOptions): string {
  const params = URL_PARAMS.filter(key => options[key] !== undefined && options[key] !== '').map(
    key => `${key}=${encodeURIComponent(String(options[key]))}`,
  );
  // The hosted preview has a second renderer that `unity=true` selects; this one is always asked for.
  params.push('unity=false');
  return `${PREVIEW_URL}?${params.join('&')}`;
}

/**
 * The hosted avatar preview in a frame. The URL carries the options the frame starts with; once
 * the frame reports it is ready it gets all of them in a message, and again when one changes.
 */
export function WearablePreview({
  id,
  onLoad,
  onError,
  onUpdate,
  blob,
  profile,
  skin,
  emote,
  camera,
  projection,
  wheelZoom,
  disableBackground,
  disableAutoRotate,
  disableFace,
  disableDefaultWearables,
}: Props) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [ready, setReady] = useState(false);
  const callbacks = useRef({ onLoad, onError, onUpdate });
  callbacks.current = { onLoad, onError, onUpdate };

  const options = useMemo(
    () =>
      withoutUndefined({
        blob,
        profile,
        skin,
        emote,
        camera,
        projection,
        wheelZoom,
        disableBackground,
        disableAutoRotate,
        disableFace,
        disableDefaultWearables,
      }),
    [
      blob,
      profile,
      skin,
      emote,
      camera,
      projection,
      wheelZoom,
      disableBackground,
      disableAutoRotate,
      disableFace,
      disableDefaultWearables,
    ],
  );
  const [url] = useState(() => buildPreviewUrl(options));

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      if (!isPreviewMessage(event, frame.current)) return;
      switch (event.data?.type) {
        case PreviewMessageType.READY:
          setReady(true);
          break;
        case PreviewMessageType.LOAD:
          callbacks.current.onLoad?.();
          break;
        case PreviewMessageType.ERROR:
          callbacks.current.onError?.(new Error(event.data.payload?.message));
          break;
      }
    };
    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  useEffect(() => {
    if (!ready || !frame.current) return;
    postToPreview(frame.current, PreviewMessageType.UPDATE, { options });
    callbacks.current.onUpdate?.();
  }, [ready, options]);

  return (
    <iframe
      ref={frame}
      id={id}
      className={styles.frame}
      src={url}
      title="Avatar preview"
      allow="autoplay"
    />
  );
}

WearablePreview.createController = createController;

function withoutUndefined(options: PreviewOptions): PreviewOptions {
  return Object.fromEntries(Object.entries(options).filter(([, value]) => value !== undefined));
}
