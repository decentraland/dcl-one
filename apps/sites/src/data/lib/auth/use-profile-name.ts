import { useEffect, useState } from "react";

import { profileFaceUrl } from "./profile-face-url";
import { isProfileOfAddress, profileDisplayName } from "./profile-label";

type ProfileIdentity = {
  name: string;
  avatarUrl: string;
};

const EMPTY: ProfileIdentity = { name: "", avatarUrl: "" };

export function useProfileIdentity(
  address: string | null | undefined,
  isConnected: boolean,
): ProfileIdentity {
  const [identity, setIdentity] = useState<ProfileIdentity & { address: string }>({ ...EMPTY, address: "" });

  useEffect(() => {
    if (!isConnected || !address) return;
    let cancelled = false;
    void (async () => {
      try {
        const { fetchProfile } = await import("../catalyst/social/profile");
        if (cancelled) return;
        const avatar = await fetchProfile(address);
        if (cancelled || !avatar || !isProfileOfAddress(avatar, address)) return;
        const name = profileDisplayName(avatar);
        const avatarUrl = profileFaceUrl(avatar.avatar?.snapshots?.face256);
        setIdentity((prev) => {
          const next = {
            address: address.toLowerCase(),
            name,
            avatarUrl,
          };
          return next.address === prev.address && next.name === prev.name && next.avatarUrl === prev.avatarUrl
            ? prev
            : next;
        });
      } catch {
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isConnected, address]);

  return isConnected && address?.toLowerCase() === identity.address ? identity : EMPTY;
}

export function useProfileName(
  address: string | null | undefined,
  isConnected: boolean,
): string {
  return useProfileIdentity(address, isConnected).name;
}
