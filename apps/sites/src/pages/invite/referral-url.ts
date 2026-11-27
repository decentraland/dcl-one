/** decentraland.org's sign-in link for an invite (sites hooks/useReferralUrl.ts): the referrer rides on the sign-in and on its redirect to the download. */
export function referralUrl(referrer: string): string {
  if (!referrer) return `/auth/login?${new URLSearchParams({ redirectTo: "/download" })}`;
  const download = `/download?${new URLSearchParams({ referrer })}`;
  return `/auth/login?${new URLSearchParams({ referrer, redirectTo: download })}`;
}
