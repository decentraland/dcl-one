import { latestPosts } from "@/data/lib/create-landing/blog-posts.server";

/** The creators landing's blog rail for a client-side visit; the server render carries it in the page loader. */
export async function loader() {
  return Response.json(await latestPosts());
}
