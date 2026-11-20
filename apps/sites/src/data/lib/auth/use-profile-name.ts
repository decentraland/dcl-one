import { useEffect, useState } from "react";
import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";

import { profileFaceUrl, profileSnapshotUrl } from "./profile-face-url";
import { isProfileOfAddress, profileDisplayName } from "./profile-label";

type ProfileIdentity = {
  name: string;
  avatarUrl: string;
  bodyUrl: string;
  /** The avatar disc's colour decentraland.org derives from the display name; empty until the profile loads. */
  avatarColor: string;
};

const EMPTY: ProfileIdentity = { name: "", avatarUrl: "", bodyUrl: "", avatarColor: "" };

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
        const avatarColor = getAvatarBackgroundColor(getDisplayName(avatar.name, avatar.hasClaimedName, avatar.ethAddress ?? address));
        setIdentity((prev) => {
          const next = {
            address: address.toLowerCase(),
            name,
            avatarUrl,
            bodyUrl: profileSnapshotUrl(avatar.avatar?.snapshots?.body, "body.png"),
            avatarColor,
          };
          return next.address === prev.address && next.name === prev.name && next.avatarUrl === prev.avatarUrl && next.bodyUrl === prev.bodyUrl && next.avatarColor === prev.avatarColor
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
