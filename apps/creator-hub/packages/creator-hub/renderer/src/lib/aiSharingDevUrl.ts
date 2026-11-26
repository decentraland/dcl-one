/**
 * Local development of AI sharing: a dev build (`pnpm run start`) started with
 * `VITE_AI_SHARING_API_URL` sends to that service instead of the JSON config's
 * `AI_SHARING_API_URL`, and turns the `creatorhub-ai-prompt-sharing` flag on
 * (see `featureFlags/overrides.ts`), so nothing has to be edited by hand.
 *
 * Packaged and release builds ignore the variable, even when it was set at
 * build time: only `import.meta.env.DEV` builds read it.
 */
export type AiSharingDevEnv = {
  DEV: boolean;
  VITE_AI_SHARING_API_URL?: string;
};

// Only the two keys, never a bare `import.meta.env`: Vite would inline the whole env object, every
// build-time `VITE_*` value, into the release bundle.
export const buildEnv = (): AiSharingDevEnv => ({
  DEV: import.meta.env.DEV,
  VITE_AI_SHARING_API_URL: import.meta.env.VITE_AI_SHARING_API_URL,
});

/** The dev build's override, or '' when there is none or this is not a dev build. */
export function devAiSharingUrl(env: AiSharingDevEnv = buildEnv()): string {
  if (!env.DEV) return '';
  return (env.VITE_AI_SHARING_API_URL ?? '').trim();
}
