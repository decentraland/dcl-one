import { ShopFooter as Footer } from "@dcl/one-ui";

async function subscribe(email: string): Promise<void> {
  const response = await fetch("/v1/newsletter", {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ email, source: "Shop" }),
  });
  await response.body?.cancel().catch(() => undefined);
  if (!response.ok) throw new Error(`Newsletter request returned ${response.status}`);
}

export default function ShopFooter() {
  return <Footer subscribe={subscribe} />;
}
