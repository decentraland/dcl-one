import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";

import type { StoryId } from "@/core/lib/telemetry/story-id";
import { track } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/index";
import { emptyProfile, normalizeAddress, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { canReadProfile } from "@/data/lib/profile/queries";

import { ProfileRouteError } from "./errors";
import { ProfileAssetsScreen } from "./ProfileAssetsScreen";
import { ProfileCommunitiesScreen } from "./ProfileCommunitiesScreen";
import { ProfileCreationsScreen } from "./ProfileCreationsScreen";
import { ProfileOverviewScreen } from "./ProfileOverviewScreen";
import { ProfilePhotosScreen } from "./ProfilePhotosScreen";
import { ProfilePlacesScreen } from "./ProfilePlacesScreen";
import { ProfileReferralsScreen } from "./ProfileReferralsScreen";
import { useProfileFlags, useRequiredProfile } from "./hooks";
import {
  isLegacyTabSegment,
  isOwnProfile,
  profileHref,
  profileTabs,
  readProfileTab,
  type ProfileTab,
} from "./tabs";

const STORY: StoryId = "landings/profile";

export type ProfilePageProps = {
  address: string;
  /** The URL's tab segment, unchecked; an unknown or hidden one goes back to Overview. */
  tabSegment: string | undefined;
  /** What the server decided from the wallet cookie, so the first render matches it. */
  ownOnServer: boolean | undefined;
  sid: string;
};

/** The signed-in wallet once the page has mounted; until then the server's reading of the cookie. */
function useOwnProfile(address: string, ownOnServer: boolean | undefined): boolean {
  const { isConnected, address: wallet } = useAuth();
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  if (!mounted && ownOnServer !== undefined) return ownOnServer;
  return isConnected && isOwnProfile(address, wallet);
}

export function ProfilePage({ address: rawAddress, tabSegment, ownOnServer, sid }: ProfilePageProps) {
  const address = normalizeAddress(rawAddress);
  if (!canReadProfile(address)) throw new ProfileRouteError("not-found", rawAddress);

  const own = useOwnProfile(address, ownOnServer);
  // Referral Rewards follows its rollout flag. Until the flags have been read, a client navigation
  // straight to that tab keeps it, also while a failed read is made again or after every try failed:
  // only a flag read as off sends the page to Overview.
  const flags = useProfileFlags(own);
  const flagsUnread =
    own && !flags.isSuccess && readProfileTab(tabSegment, own, { referral: true }) === "referral-rewards";
  const referral = flags.data?.referral === true || flagsUnread;
  const tab = readProfileTab(tabSegment, own, { referral });
  const navigate = useNavigate();
  const legacySegment = isLegacyTabSegment(tabSegment);
  useEffect(() => {
    if (tab === null) navigate(profileHref(address), { replace: true });
    // An old tab id in the URL moves to the current one, as the server does on a document request.
    else if (legacySegment) navigate(profileHref(address, tab), { replace: true });
  }, [tab, legacySegment, address, navigate]);

  const query = useRequiredProfile(address);
  const profile = query.data ?? emptyProfile(address);
  useProfileViewed(sid, address, own, query.isSuccess ? (query.data ? "live" : "fallback") : null, profile);

  const activeTab: ProfileTab = tab ?? "overview";
  const tabs = profileTabs(own, { referral }).map((entry) => ({ id: entry.id, label: entry.label }));
  const onTab = (next: string) => track("profile_tab_changed", { tab: next as ProfileTab }, { sid, story: STORY });

  const navigation = { tabs: tabs.map((entry) => ({ ...entry, href: profileHref(address, entry.id) })) };

  // Every tab has the profile site's layout: its own tab bar (scrollable on narrow screens) and no
  // section side panel.
  if (activeTab === "overview") {
    return (
      <ProfileOverviewScreen
        profile={profile}
        hasProfile={Boolean(query.data)}
        own={own}
        tabs={navigation.tabs}
        onTab={onTab}
      />
    );
  }

  if (activeTab === "creations") {
    // Keyed by address: another profile starts on its default category with no pending focus.
    return <ProfileCreationsScreen key={profile.address} profile={profile} own={own} tabs={navigation.tabs} onTab={onTab} />;
  }

  if (activeTab === "assets") {
    return <ProfileAssetsScreen key={profile.address} profile={profile} own={own} tabs={navigation.tabs} onTab={onTab} />;
  }

  if (activeTab === "photos") {
    return <ProfilePhotosScreen profile={profile} own={own} tabs={navigation.tabs} onTab={onTab} />;
  }

  if (activeTab === "places") {
    return (
      <ProfilePlacesScreen
        profile={profile}
        // A failed background re-read keeps the profile; only a first read still running is unknown.
        profileReady={query.data !== undefined}
        own={own}
        tabs={navigation.tabs}
        onTab={onTab}
        sid={sid}
      />
    );
  }

  if (activeTab === "referral-rewards") {
    return <ProfileReferralsScreen profile={profile} own={own} tabs={navigation.tabs} onTab={onTab} />;
  }

  // Keyed by address: another profile reports its own failed read.
  return <ProfileCommunitiesScreen key={profile.address} profile={profile} own={own} tabs={navigation.tabs} onTab={onTab} sid={sid} />;
}

function useProfileViewed(
  sid: string,
  address: string,
  own: boolean,
  source: "live" | "fallback" | null,
  profile: ProfileVM,
) {
  const last = useRef<string | null>(null);
  useEffect(() => {
    if (source === null) return;
    const key = `${address}|${source}|${own}`;
    if (last.current === key) return;
    last.current = key;
    track(
      "profile_viewed",
      { address, has_claimed_name: profile.hasClaimedName ?? false, source, own },
      { sid, story: STORY },
    );
  }, [sid, address, source, own, profile.hasClaimedName]);
}
