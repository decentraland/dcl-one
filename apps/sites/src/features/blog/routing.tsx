import { Form, Link } from "react-router";

import type { BlogFormProps, BlogLinkProps, BlogRouting } from "@dcl/one-ui/apps/sites/blog";

/** The blog's links and search form through React Router: client-side once hydrated, plain `<a>`/GET `<form>` before. */
export const blogRouting: BlogRouting = {
  Link: ({ href, ...rest }: BlogLinkProps) => <Link to={href} {...rest} />,
  Form: ({ action, method, ...rest }: BlogFormProps) => <Form action={action} method={method} {...rest} />,
};

const POST_HREF = /^\/blog\/([^/]+)$/;

/** The slug of the post link a click landed on, or null when the click was not on one. */
export function clickedPostSlug(event: { target: EventTarget | null }): string | null {
  const anchor = event.target instanceof Element ? event.target.closest("a") : null;
  const m = POST_HREF.exec(anchor?.getAttribute("href") ?? "");
  return m ? m[1] : null;
}
