import { fetchReferrerProfile, referrerFromAddress, resolveReferrerAddress, type ReferrerVM } from "./referral";

/** The invite's referrer: the wallet its NAME or address names, with the profile's name and face when the node has them. */
export async function loadInviteReferral(referrerParam: string, signal?: AbortSignal): Promise<{ referrer: ReferrerVM }> {
  const address = await resolveReferrerAddress(referrerParam.trim(), { signal });
  const referrer = address
    ? await fetchReferrerProfile(address, { signal }).catch(() => referrerFromAddress(address))
    : referrerFromAddress(null);
  return { referrer };
}
