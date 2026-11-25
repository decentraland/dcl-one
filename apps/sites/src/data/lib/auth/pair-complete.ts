const PAIR_API_PATH = "/internal/pair";

/**
 * Hands the phone's signature to the waiting computer. Every answer's body is either read or released, so a
 * refused or expired code does not hold its connection open.
 */
export async function completePairing(id: string, signer: string, signature: string): Promise<void> {
  const res = await fetch(PAIR_API_PATH, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ kind: "complete", id, signer, signature }),
  });
  if (res.ok) {
    await res.body?.cancel().catch(() => {});
    return;
  }
  if (res.status === 409 || res.status === 410 || res.status === 404) {
    await res.body?.cancel().catch(() => {});
    throw new Error(res.status === 409 ? "This code was already used." : "This code expired \u{2014} scan a fresh one.");
  }
  const body = (await res.json().catch(() => null)) as { error?: string } | null;
  throw new Error(body?.error ?? `The sign-in couldn't be completed (${res.status}).`);
}
