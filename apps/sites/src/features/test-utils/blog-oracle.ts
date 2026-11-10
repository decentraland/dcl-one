// The scoped normalisations every blog oracle comparison applies (blog plan B0 oracles, R5-1):
// - R4-4: an image whose asset has a title carries it as alt (the source leaves alt empty);
// - N3: an X post is compared by its post id (the source's tree holds the iframe X's widgets script
//   injects after load; ours holds the container the script fills). Seven X posts rendered nothing in
//   the source capture (X's script filled no iframe for any post of two articles, while all 5 of a
//   third rendered); for exactly those ids our container is dropped, so the rest still compares.
// - O5 (held): the source prints the library's debug text for an asset-hyperlink; ours renders the
//   link's text as any node outside the supported set does. Each debug text is replaced by that
//   node's text, so the comparison pins both outputs until the owner decides.
import type { BlogPost, RichTextNode } from "@/data/lib/blog/types";

import type { CanonicalNode } from "./legal-canonical";

/** X posts that rendered nothing in the source capture (no iframe at their place in the oracle). */
export const UNAVAILABLE_AT_CAPTURE: ReadonlySet<string> = new Set([
  "1753144917130707349",
  "1773484586447867941",
  "2035177225599991919",
  "2038754257936191542",
  "2039805009458499803",
  "2039816730248405375",
  "2039836996714308092",
]);

/** N3, on our render: each X post container becomes a marker element carrying the post id. */
export function markTweets(container: Element): void {
  for (const tweet of container.querySelectorAll("[data-tweet-id]")) {
    const id = tweet.getAttribute("data-tweet-id")!;
    if (UNAVAILABLE_AT_CAPTURE.has(id)) {
      tweet.remove();
      continue;
    }
    const marker = container.ownerDocument.createElement("x-tweet");
    marker.setAttribute("id", id);
    tweet.replaceWith(marker);
  }
}

/** R4-4, N3 and O5, on the source tree, for the post it renders. */
export function normaliseSourceTree(
  tree: CanonicalNode[],
  post: BlogPost,
): CanonicalNode[] {
  const titles = new Map(
    Object.values(post.bodyAssets).map((asset) => [asset.url, asset.title]),
  );
  // O5: the text of each asset-hyperlink node, by its asset id.
  const held = new Map<string, string>();
  const text = (node: RichTextNode): string =>
    (node.value ?? "") + (node.content ?? []).map(text).join("");
  const collect = (node: RichTextNode) => {
    if (node.nodeType === "asset-hyperlink")
      held.set(
        (node.data?.target as { sys: { id: string } }).sys.id,
        text(node),
      );
    (node.content ?? []).forEach(collect);
  };
  if (post.body) collect(post.body);
  const fix = (node: CanonicalNode): CanonicalNode => {
    if ("text" in node) {
      return {
        text: node.text.replace(
          /type: asset-hyperlink id: (\w+)/g,
          (debug, id: string) => held.get(id) ?? debug,
        ),
      };
    }
    if (!("children" in node)) return node;
    if (node.tag === "iframe" && node.src) {
      const url = new URL(node.src);
      if (
        url.host === "platform.twitter.com" &&
        url.pathname === "/embed/Tweet.html"
      )
        return {
          tag: "x-tweet",
          id: url.searchParams.get("id") ?? "",
          children: [],
        };
    }
    if (
      node.tag === "img" &&
      node.alt === "" &&
      node.src &&
      titles.get(node.src)
    )
      return {
        ...node,
        alt: titles.get(node.src)!,
        children: node.children.map(fix),
      };
    return {
      ...node,
      children: node.children
        .map(fix)
        .filter((child) => !("text" in child) || child.text !== ""),
    };
  };
  return tree.map(fix);
}
