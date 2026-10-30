import { useEffect, useState, type RefObject } from "react";

export async function copyText(text: string): Promise<void> {
  if (navigator.clipboard && window.isSecureContext) return navigator.clipboard.writeText(text);
  window.prompt("Copy to clipboard:", text);
}

function relativeTime(iso: string): string {
  const elapsed = (new Date(iso).getTime() - Date.now()) / 1000, absolute = Math.abs(elapsed);
  const units: Array<[Intl.RelativeTimeFormatUnit, number]> = [["year", 31536000], ["month", 2592000], ["day", 86400], ["hour", 3600], ["minute", 60]];
  const [unit, divisor] = units.find(([, n]) => absolute >= n) ?? ["second", 1];
  return `Last updated ${new Intl.RelativeTimeFormat("en", { numeric: "auto" }).format(Math.round(elapsed / divisor), unit)}`;
}

/** "Last updated …", computed in the browser so the server render never depends on the clock. */
export function useLastUpdated(iso: string | null): string {
  const [label, setLabel] = useState("");
  useEffect(() => { if (iso) setLabel(relativeTime(iso)); }, [iso]);
  return label;
}

/**
 * Behaviour the rendered Markdown body needs: copy buttons on code blocks, click-to-load
 * YouTube embeds (no request until Play), <details> anchors that do not toggle, and the
 * outline entry of the heading in view.
 */
export function useDocsBody(body: RefObject<HTMLElement | null>, toc: RefObject<HTMLElement | null>, key: string): void {
  useEffect(() => {
    const root = body.current;
    if (!root) return;
    const cleanups: Array<() => void> = [];
    for (const pre of root.querySelectorAll("pre")) {
      if (pre.parentElement?.classList.contains("code-block")) continue;
      const wrap = document.createElement("div"), button = document.createElement("button");
      wrap.className = "code-block";
      button.className = "code-copy";
      button.type = "button";
      button.textContent = "Copy";
      button.setAttribute("aria-label", "Copy code");
      pre.before(wrap);
      wrap.append(pre, button);
      button.addEventListener("click", async () => {
        await copyText(pre.innerText);
        button.textContent = "Copied";
        setTimeout(() => { button.textContent = "Copy"; }, 1400);
      });
    }
    for (const anchor of root.querySelectorAll<HTMLAnchorElement>(".details-anchor")) {
      const stop = (event: Event) => event.stopPropagation();
      anchor.addEventListener("click", stop);
      cleanups.push(() => anchor.removeEventListener("click", stop));
    }
    for (const button of root.querySelectorAll<HTMLButtonElement>(".video-play")) {
      const play = () => {
        const frame = document.createElement("iframe"), url = new URL(button.dataset.videoSrc ?? "");
        url.searchParams.set("autoplay", "1");
        frame.src = url.href;
        frame.title = "YouTube video";
        frame.width = "768";
        frame.height = "432";
        frame.allow = "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share";
        frame.allowFullscreen = true;
        button.replaceWith(frame);
        frame.focus();
      };
      button.addEventListener("click", play, { once: true });
      cleanups.push(() => button.removeEventListener("click", play));
    }
    const links = [...(toc.current?.querySelectorAll<HTMLAnchorElement>(".toc-links a") ?? [])];
    if (links.length && "IntersectionObserver" in window) {
      const byId = new Map(links.map((a) => [decodeURIComponent(a.hash.slice(1)), a]));
      const observer = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          links.forEach((a) => a.classList.toggle("active", a === byId.get(entry.target.id)));
          break;
        }
      }, { rootMargin: "-120px 0px -70%" });
      byId.forEach((_, id) => { const heading = document.getElementById(id); if (heading) observer.observe(heading); });
      cleanups.push(() => observer.disconnect());
    }
    return () => cleanups.forEach((fn) => fn());
  }, [body, toc, key]);
}
