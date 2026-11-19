/**
 * The rollout flag the profile site read before it showed Referral Rewards: `show-referral` of the
 * `dapps` application.
 */
export const REFERRAL_FLAG = "dapps-show-referral";

/** The rollout flags a profile page reads, each on only where the flags service says `true`. */
export type ProfileFlags = { referral: boolean };

export function profileFlagsFrom(flags: Readonly<Record<string, unknown>> | null | undefined): ProfileFlags {
  return { referral: flags?.[REFERRAL_FLAG] === true };
}

/**
 * The flags as this site's server reads them (`/internal/dapps-flags`): the flags service answers by
 * the asking site, so a browser on another domain would be told every flag is off. Browser only; a
 * document request gets them from the profile loader.
 */
export async function fetchProfileFlags(opts: { signal?: AbortSignal } = {}): Promise<ProfileFlags> {
  const res = await fetch("/internal/dapps-flags", { signal: opts.signal, headers: { accept: "application/json" } });
  if (!res.ok) {
    await res.body?.cancel().catch(() => {});
    throw new Error(`the flags route answered ${res.status}`);
  }
  return profileFlagsFrom((await res.json()) as Record<string, unknown>);
}
