import { useEffect, useState } from "react";
import { avatarBackground, displayName } from "@ui/web/places/avatarColor";

import { profileFaceUrl } from "./profile-face-url";
import { isProfileOfAddress, profileDisplayName } from "./profile-label";

type ProfileIdentity = {
  name: string;
  avatarUrl: string;
  /** The avatar disc's colour decentraland.org derives from the display name; empty until the profile loads. */
  avatarColor: string;
};

const EMPTY: ProfileIdentity = { name: "", avatarUrl: "", avatarColor: "" };

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
        const avatarColor = avatarBackground(displayName(avatar.name, avatar.hasClaimedName, avatar.ethAddress ?? address));
        setIdentity((prev) => {
          const next = {
            address: address.toLowerCase(),
            name,
            avatarUrl,
            avatarColor,
          };
          return next.address === prev.address && next.name === prev.name && next.avatarUrl === prev.avatarUrl && next.avatarColor === prev.avatarColor
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
