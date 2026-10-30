import type { ReactNode } from "react";
import { asset } from "../asset";
import DclLogomark from "../atoms/DclLogomark";
import { ART_FILES, isRetryable, statusCopy, STATUS_PAGE_STYLE } from "./statusCopy";

export type StatusPageProps = {
  status: number;
  /** Overrides the default copy for this status. */
  title?: string;
  message?: string;
  /** Technical detail (status text, stack, response body); shown only when isDev. */
  detail?: string;
  isDev?: boolean;
  homeHref?: string;
  /** Replaces the default actions (home, plus retry or back). */
  actions?: ReactNode;
};

/** The branded screen for HTTP errors (4xx and 5xx). */
export default function StatusPage({ status, title, message, detail, isDev = false, homeHref = "/", actions }: StatusPageProps) {
  const copy = statusCopy(status);
  const art = ART_FILES[copy.art];
  return (
    <>
      <style dangerouslySetInnerHTML={{ __html: STATUS_PAGE_STYLE }} />
      <main className="status-page" aria-labelledby="sp-title" data-status={status}>
        <div className="sp-card">
          <a className="sp-logo" href={homeHref}>
            <DclLogomark size={28} />
            <span>Decentraland</span>
          </a>
          <img className="sp-art" src={asset(art.path)} width={art.width} height={art.height} alt="" />
          <p className="sp-code">{status} · {copy.eyebrow}</p>
          <h1 id="sp-title" className="sp-title">{title ?? copy.title}</h1>
          <p className="sp-message">{message ?? copy.message}</p>
          <nav className="sp-actions" aria-label="Error page navigation">
            {actions ?? (
              <>
                <a className="sp-btn sp-btn-primary" href={homeHref}>Go to homepage</a>
                {isRetryable(status)
                  ? <button type="button" className="sp-btn sp-btn-secondary" onClick={() => window.location.reload()}>Try again</button>
                  : <button type="button" className="sp-btn sp-btn-secondary" onClick={() => window.history.back()}>Go back</button>}
              </>
            )}
          </nav>
          {isDev && detail ? (
            <details className="sp-dev" open>
              <summary>Developer details (dev only)</summary>
              <pre>{detail}</pre>
            </details>
          ) : null}
        </div>
      </main>
    </>
  );
}
