import { loadBlogSearchSuggestions } from "@/data/lib/blog/pages";

// The blog search dropdown's suggestions. Short-lived public cache: the same query typed by many
// visitors reaches the server's search memo, not the CMS, and a CMS failure is never cached.
export async function loader({ request }: { request: Request }) {
  const outcome = await loadBlogSearchSuggestions(request);
  if (outcome.status !== 200) {
    return Response.json({ error: "Blog search unavailable" }, { status: 503, headers: { "Cache-Control": "no-store", "Retry-After": "300" } });
  }
  return Response.json(outcome.payload, { headers: { "Cache-Control": "public, max-age=60" } });
}
