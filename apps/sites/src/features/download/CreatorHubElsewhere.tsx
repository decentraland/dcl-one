import { useEffect, useRef, useState } from "react";

import { Button } from "@dcl/one-ui";
import { docsUrl } from "@web-core/docs";

import { track } from "@/core/lib/telemetry/track";

import { CREATOR_HUB_DOWNLOAD_STORY } from "./creator-hub-click";
import styles from "./CreatorHubElsewhere.module.css";

const copy = {
  note: "Creator Hub is a desktop app for macOS and Windows. Download it on your computer.",
  docs: "Explore the Creator Docs",
  share: "Share link",
  copied: "Link copied",
  shareTitle: "Decentraland Creator Hub",
};

const COPIED_MS = 2000;

/** What a visitor on a system without an installer (a phone, a tablet, Linux) can do instead of downloading. */
export default function CreatorHubElsewhere({ sid }: { sid: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current);
  }, []);

  const report = (method: "share" | "copy") =>
    track("lp_creatorhub_link_shared", { method }, { sid, story: CREATOR_HUB_DOWNLOAD_STORY });

  const share = async () => {
    // The page's own address without the query: a link that named this device's system would offer nothing elsewhere.
    const url = `${window.location.origin}${window.location.pathname}`;
    if (typeof navigator.share === "function") {
      try {
        await navigator.share({ title: copy.shareTitle, url });
        report("share");
        return;
      } catch (error) {
        // A dismissed sheet is the visitor's choice; any other refusal (a webview, a lost gesture) falls back to copying.
        if (error instanceof DOMException && error.name === "AbortError") return;
      }
    }
    try {
      await navigator.clipboard.writeText(url);
    } catch {
      return;
    }
    report("copy");
    setCopied(true);
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), COPIED_MS);
  };

  return (
    <div className={styles.root}>
      <p className={styles.note}>{copy.note}</p>
      <div className={styles.actions}>
        <Button
          size="lg"
          href={docsUrl("creator")}
          onClick={() => track("lp_creatorhub_docs_clicked", {}, { sid, story: CREATOR_HUB_DOWNLOAD_STORY })}
        >
          {copy.docs}
        </Button>
        <Button size="lg" variant="outline" onClick={share} aria-live="polite">
          {copied ? copy.copied : copy.share}
        </Button>
      </div>
    </div>
  );
}
