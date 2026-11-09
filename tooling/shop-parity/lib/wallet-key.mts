// The test wallet's key for the signed-in pages, read from the environment and never from the repository.
// Kept apart from the session (which signs with viem) so the parts every run reads need nothing installed.

export const WALLET_KEY_ENV = "SHOP_PARITY_WALLET_KEY";

type Hex = `0x${string}`;

/** The test wallet's key, or a refusal naming the variable: a signed-in page never runs signed out. */
export function walletKey(env: NodeJS.ProcessEnv = process.env): Hex {
  const key = env[WALLET_KEY_ENV]?.trim();
  if (!key || !/^0x[0-9a-fA-F]{64}$/.test(key)) {
    throw new Error(`${WALLET_KEY_ENV} must hold the test wallet's 0x-prefixed private key for the signed-in pages`);
  }
  return key as Hex;
}

/**
 * The pages a run can take: without the test wallet's key the signed-in ones are left out, said once,
 * so a run without it still compares (and reports) every signed-out page.
 */
export function runnable<T extends { id: string; signedIn?: boolean }>(specs: T[], env: NodeJS.ProcessEnv = process.env, log: (line: string) => void = console.warn): T[] {
  if (env[WALLET_KEY_ENV]?.trim()) return specs;
  const skipped = specs.filter((s) => s.signedIn);
  if (skipped.length > 0) log(`${WALLET_KEY_ENV} is not set: skipping the signed-in pages (${skipped.map((s) => s.id).join(", ")})`);
  return specs.filter((s) => !s.signedIn);
}
