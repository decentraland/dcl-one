import { useEffect, useState } from "react";

import { asset } from "../asset";
import styles from "./HeroVideo.module.css";

const DESKTOP_QUERY = "(min-width: 900px)";
const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

type IdleWindow = Window & {
  requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
  cancelIdleCallback?: (handle: number) => void;
};

/**
 * Decorative looping background video layered over the hero's static image.
 *
 * The static <img> stays the LCP element: the video is only mounted on desktop
 * widths, when the user has not asked for reduced motion, and once the browser
 * is idle. It fades in after `canplay`, so nothing shifts or flashes.
 */
export default function HeroVideo() {
  const [load, setLoad] = useState(false);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    if (!window.matchMedia(DESKTOP_QUERY).matches) return;
    if (window.matchMedia(REDUCED_MOTION_QUERY).matches) return;
    const w = window as IdleWindow;
    if (w.requestIdleCallback) {
      const handle = w.requestIdleCallback(() => setLoad(true), { timeout: 4000 });
      return () => w.cancelIdleCallback?.(handle);
    }
    const timer = window.setTimeout(() => setLoad(true), 3000);
    return () => window.clearTimeout(timer);
  }, []);

  if (!load) return null;
  return (
    <video
      className={styles.root}
      autoPlay
      loop
      muted
      playsInline
      preload="none"
      aria-hidden="true"
      tabIndex={-1}
      onCanPlay={(e) => e.currentTarget.setAttribute("data-ready", "true")}
    >
      <source src={asset("assets/home-hero.webm")} type="video/webm" />
      <source src={asset("assets/home-hero.mp4")} type="video/mp4" />
    </video>
  );
}
