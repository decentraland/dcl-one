// The shared site top bar over the docs chrome: SUMMARY.md sidebar, the rendered page, its outline, page navigation and the theme switcher.
import { useEffect, useRef, useState } from "react";

import DclTopBar from "@ui/web/frames/DclTopBar";
import type { DocsPageView, DocsSpace } from "@/data/lib/docs/types";
import DocsIcon from "./DocsIcon";
import { NavList } from "./DocsNav";
import DocsSearch, { type DocsSearchHandle } from "./DocsSearch";
import { copyText, useDocsBody, useLastUpdated } from "./useDocsBody";
import "./docs.css";

type Theme = "light" | "system" | "dark";
const THEME_KEY = "dcl-docs-theme";
// Runs before the first paint so a dark reader never sees a light flash.
const THEME_BOOT = `try{var c=localStorage.getItem('${THEME_KEY}')||'system',d=c==='dark'||c==='system'&&matchMedia('(prefers-color-scheme:dark)').matches;document.documentElement.dataset.docsTheme=d?'dark':'light';document.documentElement.dataset.docsThemeChoice=c}catch(e){}`;
const LANGUAGES: Array<[string, string]> = [
  ["ES", "https://docs.decentraland.org/player/player-es/"],
  ["PT", "https://docs.decentraland.org/player/player-pt/"],
  ["KO", "https://docs.decentraland.org/player/player-ko/"],
  ["ZH", "https://docs.decentraland.org/player/player-zh/"],
];

const spaceHome = (space: DocsSpace) => (space.id === "player" ? "/docs/" : `/docs/${space.id}/`);

function applyTheme(choice: Theme) {
  const dark = choice === "dark" || (choice === "system" && matchMedia("(prefers-color-scheme:dark)").matches);
  document.documentElement.dataset.docsTheme = dark ? "dark" : "light";
  document.documentElement.dataset.docsThemeChoice = choice;
}

export default function DocsPage({ view }: { view: DocsPageView }) {
  const nav = useRef<HTMLElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const toc = useRef<HTMLElement>(null);
  const search = useRef<DocsSearchHandle>(null);
  const [drawer, setDrawer] = useState<"" | "nav" | "toc">("");
  const [mobileSearch, setMobileSearch] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [copied, setCopied] = useState("");
  const [theme, setTheme] = useState<Theme | null>(null);
  const updated = useLastUpdated(view.updatedAt);
  useDocsBody(body, toc, view.url);

  useEffect(() => {
    setTheme((document.documentElement.dataset.docsThemeChoice as Theme | undefined) ?? "system");
    const active = nav.current?.querySelector<HTMLElement>("a.active");
    const sidebar = nav.current;
    if (active && sidebar) requestAnimationFrame(() => {
      const a = active.getBoundingClientRect(), n = sidebar.getBoundingClientRect();
      sidebar.scrollTop = Math.max(0, sidebar.scrollTop + a.top - n.top - sidebar.clientHeight * 0.45);
    });
  }, [view.url]);

  useEffect(() => {
    if (!theme) return;
    applyTheme(theme);
    try { localStorage.setItem(THEME_KEY, theme); } catch {}
    const query = matchMedia("(prefers-color-scheme:dark)");
    const onChange = () => { if (theme === "system") applyTheme("system"); };
    query.addEventListener?.("change", onChange);
    return () => query.removeEventListener?.("change", onChange);
  }, [theme]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        if (window.innerWidth <= 760) setMobileSearch(true);
        search.current?.focus();
      } else if (event.key === "Escape") {
        setDrawer("");
        setMenuOpen(false);
        document.querySelector(".dcl-docs .language")?.removeAttribute("open");
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (event: PointerEvent) => {
      if (!(event.target as Element).closest?.(".more-wrap")) setMenuOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [menuOpen]);

  useEffect(() => { if (mobileSearch) search.current?.focus(); }, [mobileSearch]);

  const flash = (what: string) => { setCopied(what); setTimeout(() => setCopied(""), 1400); };
  const copyMarkdown = async (what: string) => {
    try {
      const response = await fetch(view.mdUrl);
      if (!response.ok) throw new Error("Markdown unavailable");
      await copyText(await response.text());
      flash(what);
    } catch {
      window.prompt("Copy this page:", document.querySelector(".dcl-docs main")?.textContent ?? "");
    }
    setMenuOpen(false);
  };
  const ask = encodeURIComponent(`Read ${view.title} in the Decentraland documentation and help me understand it: ${view.mdUrl}`);
  const closeDrawer = () => setDrawer("");

  return (
    <>
      <div className="ui2 docs-topbar"><DclTopBar active="learn" /></div>
      <div className="dcl-docs">
        <script dangerouslySetInnerHTML={{ __html: THEME_BOOT }} />
        <header className="site-header">
          <div className="spacebar">
            <button className="menu-toggle" type="button" aria-label="Open table of contents" aria-expanded={drawer === "nav"} onClick={() => setDrawer(drawer === "nav" ? "" : "nav")}>
              <DocsIcon name="menu" />
            </button>
            <nav className="spaces" aria-label="Documentation spaces">
              {view.spaces.map((space) => (
                <a key={space.id} href={spaceHome(space)} className={space.id === view.space.id ? "active" : undefined} aria-current={space.id === view.space.id ? "page" : undefined}>
                  <DocsIcon name={space.icon} />
                  {space.title}
                </a>
              ))}
            </nav>
            <DocsSearch ref={search} mobileOpen={mobileSearch} />
            <details className="language">
              <summary aria-label="Language">
                <DocsIcon name="globe" />
                <span className="language-label">EN</span>
                <DocsIcon name="chevron" className="icon chevron" />
              </summary>
              <div className="language-menu" role="menu">
                <a href="/docs/" aria-current="true">EN</a>
                {LANGUAGES.map(([code, href]) => <a key={code} href={href}>{code}</a>)}
              </div>
            </details>
            <button className="mobile-search" type="button" aria-label="Open search" aria-expanded={mobileSearch} onClick={() => { if (mobileSearch) search.current?.close(); setMobileSearch(!mobileSearch); }}>
              <DocsIcon name="search" />
            </button>
          </div>
        </header>
        <button className={drawer ? "drawer-scrim open" : "drawer-scrim"} type="button" aria-label="Close menus" onClick={closeDrawer} />
        <div className="layout">
          <aside className={drawer === "nav" ? "sidebar open" : "sidebar"} aria-label="Section navigation" ref={nav}>
            <NavList node={view.nav} current={view.navUrl} onNavigate={closeDrawer} />
          </aside>
          <main className="content">
            <div className="page-head">
              <div className="page-tools">
                <button className="tool copy-page" type="button" title="Copy page as Markdown" aria-label="Copy" onClick={() => copyMarkdown("page")}>
                  <DocsIcon name="copy" />
                  <span>{copied === "page" ? "Copied" : "Copy"}</span>
                </button>
                <span className="more-wrap">
                  <button className="tool more-toggle" type="button" title="More actions" aria-label="More" aria-haspopup="menu" aria-expanded={menuOpen} onClick={() => setMenuOpen(!menuOpen)}>
                    <DocsIcon name="chevron" />
                  </button>
                  <span className="copy-menu" role="menu" hidden={!menuOpen}>
                    <button type="button" onClick={() => copyMarkdown("menu")}>{copied === "menu" ? "Copied" : <>Copy page <small>&nbsp;as Markdown</small></>}</button>
                    <a role="menuitem" href={view.mdUrl}>View as Markdown</a>
                    <a role="menuitem" href={`https://chatgpt.com/?q=${ask}`} target="_blank" rel="noopener">Open in ChatGPT</a>
                    <a role="menuitem" href={`https://claude.ai/new?q=${ask}`} target="_blank" rel="noopener">Open in Claude</a>
                    <button type="button" onClick={async () => { await copyText("https://docs.decentraland.org/~gitbook/mcp"); flash("mcp"); }}>
                      {copied === "mcp" ? "Copied" : "Copy MCP endpoint"}
                    </button>
                    {view.editUrl ? <a role="menuitem" href={view.editUrl} target="_blank" rel="noopener">Edit on GitHub</a> : null}
                  </span>
                </span>
                <button className="tool toc-toggle" type="button" title="On this page" aria-label="On this page" aria-expanded={drawer === "toc"} onClick={() => setDrawer(drawer === "toc" ? "" : "toc")}>
                  <DocsIcon name="toc" />
                </button>
              </div>
              <nav className="breadcrumb" aria-label="Breadcrumb">
                {view.breadcrumb.map((crumb, i) => (
                  <span key={`${crumb.title}-${i}`} style={{ display: "contents" }}>
                    {i > 0 ? <span className="crumb-sep"><DocsIcon name="chevron" /></span> : null}
                    {crumb.url ? (
                      <a href={crumb.url}>{i === 0 ? <DocsIcon name={view.space.icon} /> : null}{crumb.title}</a>
                    ) : (
                      <span>{crumb.title}</span>
                    )}
                  </span>
                ))}
              </nav>
            </div>
            <div style={{ display: "contents" }} ref={body} dangerouslySetInnerHTML={{ __html: view.html }} />
            {view.prev || view.next ? (
              <nav className="page-nav" aria-label="Page navigation">
                {view.prev ? <a className="page-nav-prev" rel="prev" href={view.prev.url}><small>Previous</small><strong>{view.prev.title}</strong></a> : null}
                {view.next ? <a className="page-nav-next" rel="next" href={view.next.url}><small>Next</small><strong>{view.next.title}</strong></a> : null}
              </nav>
            ) : null}
            {view.updatedAt ? <p className="last-updated"><time dateTime={view.updatedAt}>{updated}</time></p> : null}
          </main>
          <aside className={drawer === "toc" ? "page-toc open" : "page-toc"} aria-label="On this page" ref={toc}>
            {view.toc.length ? (
              <>
                <strong>On this page</strong>
                <div className="toc-links">
                  {view.toc.map((entry) => (
                    <a key={entry.id} className={`depth-${entry.depth}`} href={`#${entry.id}`} onClick={closeDrawer}>{entry.text}</a>
                  ))}
                </div>
              </>
            ) : null}
          </aside>
        </div>
        <div className="theme-switcher" aria-label="Theme">
          {(["light", "system", "dark"] as const).map((choice) => (
            <button key={choice} type="button" className={theme === choice ? "active" : undefined} aria-label={`Switch to ${choice} theme`} onClick={() => setTheme(choice)}>
              <DocsIcon name={choice === "light" ? "sun" : choice === "system" ? "monitor" : "moon"} />
            </button>
          ))}
        </div>
      </div>
    </>
  );
}
