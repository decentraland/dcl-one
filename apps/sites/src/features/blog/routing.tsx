import { Form, Link } from "react-router";

import type { BlogFormProps, BlogLinkProps, BlogRouting } from "@dcl/one-ui/apps/sites/blog";

/** The blog's links and search form through React Router: client-side once hydrated, plain `<a>`/GET `<form>` before. */
export const blogRouting: BlogRouting = {
  Link: ({ href, ...rest }: BlogLinkProps) => <Link to={href} {...rest} />,
  Form: ({ action, method, ...rest }: BlogFormProps) => <Form action={action} method={method} {...rest} />,
};

// `/blog/<category>/<post>`; `/blog/author/<slug>` is an author's list, not a post.
const POST_HREF = /^\/blog\/(?!author\/)[^/?#]+\/([^/?#]+)$/;

/** The slug of the post card a click landed on (its `data-blog-card` link), or null for any other click. */
export function clickedPostSlug(event: { target: EventTarget | null }): string | null {
  const anchor = event.target instanceof Element ? event.target.closest("a[data-blog-card]") : null;
  const m = POST_HREF.exec(anchor?.getAttribute("href") ?? "");
  if (!m) return null;
  try {
    return decodeURIComponent(m[1]);
  } catch {
    return m[1];
  }
}
