import { siteHost } from "./site";

export async function subscribeSiteNewsletter(email: string): Promise<void> {
  const response = await fetch("/v1/newsletter", {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ email, source: siteHost() }),
  });
  // Nothing in the answer is read, on success or failure: its body is let go either way.
  await response.body?.cancel().catch(() => undefined);
  if (!response.ok) throw new Error(`Newsletter request returned ${response.status}`);
}
