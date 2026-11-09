export function validateDocs(pages: Map<string, string>, assets: Set<string>): string[] {
  const problems: string[] = [];
  for (const [page, markdown] of pages) {
    if (/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/.test(markdown)) problems.push(`control character: ${page}`);
    const visible = markdown.replace(/<!--[\s\S]*?-->|```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`/g, "");
    for (const match of visible.matchAll(/(?:\]\(\s*<?|(?:src|href|poster|data-src)=["'])((?:\/docs\/)[^\s)"'<>]*)/g)) {
      const url = match[1].split(/[?#]/, 1)[0];
      let decoded: string;
      try { decoded = decodeURIComponent(url); } catch { problems.push(`invalid URL: ${page} ${url}`); continue; }
      if (decoded.endsWith("/") ? !pages.has(decoded) : !assets.has(decoded.slice("/docs/".length)))
        problems.push(`broken local reference: ${page} ${url}`);
    }
  }
  return problems;
}
