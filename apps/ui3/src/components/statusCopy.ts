// Copy and art for HTTP error screens, shared by the React StatusPage and by
// scripts/gen-error-pages.mts, which bundles this module (no JSX, no DOM) to
// build the static pages nginx serves when the app itself cannot answer.

import { DCL_LOGOMARK_PATHS } from "../atoms/dclLogomarkPaths";

export type ErrorArt = "lost" | "locked" | "wave" | "crew";

export type StatusCopy = {
  status: number;
  eyebrow: string;
  title: string;
  message: string;
  art: ErrorArt;
};

export const ART_FILES: Record<ErrorArt, { path: string; width: number; height: number }> = {
  lost: { path: "assets/errors/lost.webp", width: 240, height: 128 },
  locked: { path: "assets/errors/locked.webp", width: 160, height: 154 },
  wave: { path: "assets/errors/wave.webp", width: 324, height: 176 },
  crew: { path: "assets/errors/crew.webp", width: 560, height: 358 },
};

const COPY: Record<number, Omit<StatusCopy, "status">> = {
  400: {
    eyebrow: "Bad request",
    title: "That request didn't quite work",
    message: "Something in the link or form was malformed. Check it and try again.",
    art: "wave",
  },
  401: {
    eyebrow: "Sign in required",
    title: "Sign in to continue",
    message: "This page needs you to be signed in. Sign in and try again.",
    art: "locked",
  },
  403: {
    eyebrow: "Access denied",
    title: "You don't have access here",
    message: "Your account can't open this page. If you think it should, ask its owner for access.",
    art: "locked",
  },
  404: {
    eyebrow: "Page not found",
    title: "Nothing built on this parcel",
    message: "We couldn't find the page you were looking for. It may have moved, or the link may be wrong.",
    art: "lost",
  },
  405: {
    eyebrow: "Not allowed",
    title: "That action isn't supported here",
    message: "This page can't handle that kind of request. Go back and try another way.",
    art: "wave",
  },
  408: {
    eyebrow: "Request timeout",
    title: "That took too long",
    message: "Your request timed out before it finished. Check your connection and try again.",
    art: "wave",
  },
  410: {
    eyebrow: "Gone",
    title: "This page has been removed",
    message: "It used to be here, but it was taken down and won't be coming back.",
    art: "lost",
  },
  413: {
    eyebrow: "Too large",
    title: "That's too big to send",
    message: "The file or form you sent is larger than we accept. Try something smaller.",
    art: "wave",
  },
  414: {
    eyebrow: "Link too long",
    title: "That link is too long",
    message: "The address is longer than we can handle. Check where it came from and try again.",
    art: "wave",
  },
  429: {
    eyebrow: "Too many requests",
    title: "Slow down a little",
    message: "You've made a lot of requests in a short time. Wait a moment, then try again.",
    art: "wave",
  },
  500: {
    eyebrow: "Server error",
    title: "Something broke on our side",
    message: "An unexpected error happened while loading this page. Try again in a moment.",
    art: "crew",
  },
  502: {
    eyebrow: "Bad gateway",
    title: "We can't reach Decentraland right now",
    message: "A service behind this page didn't answer properly. Try again in a moment.",
    art: "crew",
  },
  503: {
    eyebrow: "Service unavailable",
    title: "We'll be right back",
    message: "Decentraland is busy or down for maintenance. Try again in a few minutes.",
    art: "crew",
  },
  504: {
    eyebrow: "Gateway timeout",
    title: "That's taking longer than it should",
    message: "A service behind this page didn't answer in time. Try again in a moment.",
    art: "crew",
  },
};

/** Statuses that get a dedicated static page for nginx. */
export const STATIC_ERROR_STATUSES = Object.keys(COPY).map(Number);

export function statusCopy(status: number): StatusCopy {
  const exact = COPY[status];
  if (exact) return { status, ...exact };
  if (status >= 500) {
    return { status, eyebrow: `Error ${status}`, title: "Something went wrong on our side", message: "This page couldn't load because of a problem on our servers. Try again in a moment.", art: "crew" };
  }
  return { status, eyebrow: `Error ${status}`, title: "We couldn't open that page", message: "Something about this request didn't work. Go back and try again.", art: "wave" };
}

/** 5xx pages offer "Try again"; client errors only link home and back. */
export const isRetryable = (status: number): boolean => status >= 500 || status === 408 || status === 429;

export const STATUS_PAGE_STYLE = `
.status-page {
  min-height: 100vh; box-sizing: border-box; margin: 0;
  display: flex; align-items: center; justify-content: center;
  padding: clamp(1.25rem, 4vw, 3rem);
  background:
    radial-gradient(90% 60% at 50% -10%, rgba(255,45,85,0.18), transparent 62%),
    radial-gradient(70% 50% at 100% 110%, rgba(162,59,255,0.14), transparent 60%),
    #0d0c11;
  color: #f4f2f7;
  font-family: Inter, system-ui, -apple-system, "Segoe UI", sans-serif;
}
.status-page *, .status-page *::before, .status-page *::after { box-sizing: border-box; }
.sp-card { width: min(560px, 100%); display: flex; flex-direction: column; align-items: center; gap: 14px; text-align: center; }
.sp-logo { display: flex; align-items: center; gap: 8px; color: #fff; text-decoration: none; font-weight: 700; font-size: 15px; letter-spacing: .01em; margin-bottom: 8px; }
.sp-logo svg { width: 28px; height: 28px; }
.sp-art { width: auto; max-width: 100%; max-height: 220px; height: auto; filter: drop-shadow(0 18px 40px rgba(255,45,85,0.25)); }
.sp-code { margin: 6px 0 0; font-size: 13px; font-weight: 700; letter-spacing: .12em; text-transform: uppercase; color: #ff7a93; }
.sp-title { margin: 0; font-size: clamp(26px, 5vw, 38px); line-height: 1.15; font-weight: 800; }
.sp-message { margin: 0; max-width: 460px; font-size: 16px; line-height: 1.5; color: rgba(255,255,255,0.78); }
.sp-actions { display: flex; flex-wrap: wrap; justify-content: center; gap: 12px; margin-top: 12px; }
.sp-btn { display: inline-flex; align-items: center; justify-content: center; min-height: 44px; padding: 0 22px; border-radius: 10px; border: 1px solid transparent; font-family: inherit; font-size: 15px; font-weight: 600; line-height: 1; text-decoration: none; cursor: pointer; }
.sp-btn-primary { background: #d80029; color: #fff; }
.sp-btn-primary:hover { background: #ff2d55; }
.sp-btn-secondary { background: transparent; color: #fff; border-color: rgba(255,255,255,0.4); }
.sp-btn-secondary:hover { background: rgba(255,255,255,0.08); }
.sp-btn:focus-visible { outline: 2px solid #fff; outline-offset: 3px; }
.sp-dev { width: 100%; margin-top: 18px; text-align: left; border: 1px dashed rgba(255,255,255,0.3); border-radius: 10px; padding: 10px 14px; }
.sp-dev summary { cursor: pointer; font-size: 13px; font-weight: 600; color: #ffb3c1; }
.sp-dev pre { margin: 10px 0 0; max-height: 280px; overflow: auto; white-space: pre-wrap; word-break: break-word; font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, monospace; color: rgba(255,255,255,0.85); }
`;

/** The Decentraland logomark as markup, for pages rendered without React. */
export const DCL_MARK_SVG = `<svg viewBox="0 0 54 54" fill="currentColor" aria-hidden="true">${DCL_LOGOMARK_PATHS.map((d) => `<path d="${d}"/>`).join("")}</svg>`;
