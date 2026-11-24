import { useEffect, useRef, useState } from "react";

import type { PhotoViewerActions } from "@dcl/one-ui";

import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";

import { downloadName, reelHref } from "./photo-view";

const SHARE_TEXT = "Happening right now in @decentraland.\n\nCome hang out 👋";

function reelShareUrl(url: string): string {
  return `https://x.com/intent/post?${new URLSearchParams({ text: SHARE_TEXT, url })}`;
}

type Feedback = { id: string; message: string; version: number; fallback?: string };

export function usePhotoActions(photo: ProfilePhoto, savePhoto: boolean, initialOrigin?: string): PhotoViewerActions {
  const [browserOrigin, setBrowserOrigin] = useState<string>();
  const [feedback, setFeedback] = useState<Feedback>();
  const [downloading, setDownloading] = useState(false);
  const download = useRef<AbortController | null>(null);
  const version = useRef(0);
  useEffect(() => setBrowserOrigin(window.location.origin), []);
  useEffect(() => () => download.current?.abort(), [photo.id]);

  const origin = initialOrigin ?? browserOrigin;
  const path = reelHref(photo.id);
  const shareUrl = origin ? new URL(path, origin).href : undefined;
  const currentFeedback = feedback?.id === photo.id ? feedback : undefined;

  useEffect(() => {
    if (!currentFeedback || currentFeedback.fallback) return;
    const timer = window.setTimeout(() => {
      setFeedback((current) => (current === currentFeedback ? undefined : current));
    }, 5_000);
    return () => window.clearTimeout(timer);
  }, [currentFeedback]);

  function announce(message: string, fallback?: string) {
    setFeedback({ id: photo.id, message, version: ++version.current, fallback });
  }

  async function copy() {
    const url = shareUrl ?? new URL(path, window.location.origin).href;
    try {
      await navigator.clipboard.writeText(url);
      announce("Link copied!");
    } catch {
      announce("Could not copy automatically. Copy this link instead.", url);
    }
  }

  async function save() {
    if (download.current && !download.current.signal.aborted) return;
    const request = new AbortController();
    download.current = request;
    setDownloading(true);
    try {
      const response = await fetch(photo.url, { signal: request.signal });
      if (!response.ok) {
        await response.body?.cancel().catch(() => {});
        throw new Error("Photo download failed");
      }
      const blob = await response.blob();
      if (!["image/jpeg", "image/png"].includes(blob.type) || !blob.size) throw new Error("Photo is unavailable");
      if (request.signal.aborted) return;
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = downloadName(photo, blob.type);
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      // Let the browser hand the object to its download manager before releasing it.
      window.setTimeout(() => URL.revokeObjectURL(url), 1_000);
      announce("Download started.");
    } catch {
      if (!request.signal.aborted) announce("Could not download this photo. Try again.");
    } finally {
      if (download.current === request) {
        download.current = null;
        setDownloading(false);
      }
    }
  }

  return {
    shareHref: shareUrl ? reelShareUrl(shareUrl) : undefined,
    onCopy: copy,
    onDownload: savePhoto ? save : undefined,
    downloading,
    announcement: currentFeedback?.message,
    announcementVersion: currentFeedback?.version,
    copyFallbackHref: currentFeedback?.fallback,
  };
}
