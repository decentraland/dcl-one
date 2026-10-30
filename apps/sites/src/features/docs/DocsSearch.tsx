import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";

import DocsIcon from "./DocsIcon";

interface SearchPage { title: string; url: string; space: string; description: string; text: string }

let index: Promise<SearchPage[]> | null = null;
function loadIndex(): Promise<SearchPage[]> {
  index ??= fetch("/docs/search-index.json").then((response) => {
    if (!response.ok) throw new Error("Search index unavailable");
    return response.json() as Promise<SearchPage[]>;
  });
  index.catch(() => { index = null; });
  return index;
}

/** Title, then description, then body matches; the dependency-free ranking the static site used. */
export function rankDocs(pages: SearchPage[], query: string): SearchPage[] {
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [];
  return pages
    .map((page) => {
      const title = page.title.toLowerCase(), description = page.description.toLowerCase(), text = page.text.toLowerCase();
      let score = 0;
      for (const term of terms) {
        if (title === term) score += 100;
        else if (title.startsWith(term)) score += 50;
        else if (title.includes(term)) score += 25;
        if (description.includes(term)) score += 8;
        if (text.includes(term)) score += 2;
      }
      return { page, score };
    })
    .filter((item) => item.score > 0)
    .sort((a, b) => b.score - a.score || a.page.title.localeCompare(b.page.title))
    .slice(0, 12)
    .map((item) => item.page);
}

export interface DocsSearchHandle { focus(): void; close(): void }

const DocsSearch = forwardRef<DocsSearchHandle, { mobileOpen: boolean }>(function DocsSearch({ mobileOpen }, ref) {
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState("Type to search all documentation.");
  const [results, setResults] = useState<SearchPage[]>([]);
  const [selected, setSelected] = useState(-1);

  const close = useCallback(() => { setOpen(false); setSelected(-1); }, []);
  useImperativeHandle(ref, () => ({ focus: () => { input.current?.focus(); input.current?.select(); }, close }), [close]);

  useEffect(() => {
    const q = query.trim();
    setSelected(-1);
    if (!q) { setResults([]); setOpen(false); return; }
    setOpen(true);
    setStatus("Searching…");
    let live = true;
    loadIndex().then((pages) => {
      if (!live) return;
      const matches = rankDocs(pages, q);
      setResults(matches);
      setStatus(matches.length ? `${matches.length} result${matches.length === 1 ? "" : "s"}` : "No matching documentation found.");
    }, () => live && setStatus("Search is unavailable. Reload the page and try again."));
    return () => { live = false; };
  }, [query]);

  useEffect(() => {
    const onDown = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) close(); };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [close]);

  useEffect(() => {
    if (selected >= 0) document.getElementById(`docs-search-result-${selected}`)?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" && results.length) { event.preventDefault(); setSelected((selected + 1) % results.length); }
    else if (event.key === "ArrowUp" && results.length) { event.preventDefault(); setSelected((selected - 1 + results.length) % results.length); }
    else if (event.key === "Enter" && results.length) { event.preventDefault(); window.location.href = results[selected < 0 ? 0 : selected].url; }
    else if (event.key === "Escape") { event.preventDefault(); close(); input.current?.blur(); }
  };

  return (
    <div className={mobileOpen ? "search mobile-open" : "search"} ref={root}>
      <DocsIcon name="search" />
      <input
        ref={input}
        type="search"
        role="combobox"
        aria-label="Ask or search"
        aria-expanded={open}
        aria-controls="docs-search-results"
        aria-autocomplete="list"
        aria-activedescendant={selected >= 0 ? `docs-search-result-${selected}` : undefined}
        placeholder="Ask or search…"
        autoComplete="off"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <span className="search-shortcut" aria-hidden="true"><kbd>Ctrl</kbd><kbd>K</kbd></span>
      <div className="search-panel" role="dialog" aria-label="Search documentation" hidden={!open}>
        <div className="search-status" role="status">{status}</div>
        <div className="search-results" id="docs-search-results" role="listbox">
          {results.map((page, i) => (
            <a key={page.url} className="search-result" id={`docs-search-result-${i}`} href={page.url} role="option" aria-selected={i === selected}>
              <small>{page.space}</small>
              <strong>{page.title}</strong>
              <span>{page.description || page.text.slice(0, 180)}</span>
            </a>
          ))}
        </div>
      </div>
    </div>
  );
});

export default DocsSearch;
