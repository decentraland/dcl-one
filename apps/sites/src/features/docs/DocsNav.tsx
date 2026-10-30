import { useState } from "react";

import { pageIconSvg } from "@/data/lib/docs/icons";
import type { DocsNavNode } from "@/data/lib/docs/types";
import DocsIcon from "./DocsIcon";

function Section({ node, current, onNavigate }: { node: DocsNavNode; current: string; onNavigate: () => void }) {
  const [expanded, setExpanded] = useState(true);
  return (
    <li className="nav-section">
      <button type="button" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>
        <span>{node.title}</span>
        <DocsIcon name="chevron" />
      </button>
      <NavList node={node} current={current} onNavigate={onNavigate} />
    </li>
  );
}

/** The space's SUMMARY.md tree: sections collapse, entries without a page stay labels. */
export function NavList({ node, current, onNavigate }: { node: DocsNavNode; current: string; onNavigate: () => void }) {
  if (!node.children.length) return null;
  return (
    <ul>
      {node.children.map((child, i) => {
        if (child.type === "section") return <Section key={`s${i}`} node={child} current={current} onNavigate={onNavigate} />;
        const icon = child.icon ? pageIconSvg(child.icon, "nav-icon") : "";
        return (
          <li key={`${child.url ?? child.title}-${i}`}>
            {child.url ? (
              <a href={child.url} className={child.url === current ? "active" : undefined} onClick={onNavigate}>
                {icon ? <span style={{ display: "contents" }} dangerouslySetInnerHTML={{ __html: icon }} /> : null}
                {child.title}
              </a>
            ) : (
              <span>{child.title}</span>
            )}
            <NavList node={child} current={current} onNavigate={onNavigate} />
          </li>
        );
      })}
    </ul>
  );
}
