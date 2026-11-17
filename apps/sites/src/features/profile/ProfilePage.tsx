import { useEffect, useRef, useState, type ComponentProps } from "react";
import { useNavigate } from "react-router";

import { ProfileNavigationContext } from "@ui/web/frames/ProfileNavigation";
import StProfileCommunitiesTab from "@ui/web/pages/StProfileCommunitiesTab";
import StProfileCreationsTab from "@ui/web/pages/StProfileCreationsTab";
import StProfileMyAssetsTab from "@ui/web/pages/StProfileMyAssetsTab";
import StProfileOverviewTab from "@ui/web/pages/StProfileOverviewTab";
import StProfilePhotosTab from "@ui/web/pages/StProfilePhotosTab";

import type { StoryId } from "@/core/lib/telemetry/story-id";
import { track } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/index";
import { emptyProfile, normalizeAddress, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { canReadProfile } from "@/data/lib/profile/queries";

import { ProfileRouteError } from "./errors";
import { ProfileOverviewScreen } from "./ProfileOverviewScreen";
import { ProfilePlacesScreen } from "./ProfilePlacesScreen";
import {
  useMyCommunities,
  useProfileAssets,
  useProfileCreations,
  useProfilePhotos,
  useRequiredProfile,
} from "./hooks";
import { isOwnProfile, profileHref, profileTabs, readProfileTab, type ProfileTab } from "./tabs";

const STORY: StoryId = "landings/profile";

type TabList = { id: string; label: string }[];
type TabProps = { address: string; profile: ProfileVM; own: boolean; tabs: TabList; sid: string };

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
  const tab = readProfileTab(tabSegment, own);
  const navigate = useNavigate();
  useEffect(() => {
    if (tab === null) navigate(profileHref(address), { replace: true });
  }, [tab, address, navigate]);

  const query = useRequiredProfile(address);
  const profile = query.data ?? emptyProfile(address);
  useProfileViewed(sid, address, own, query.isSuccess ? (query.data ? "live" : "fallback") : null, profile);

  const activeTab: ProfileTab = tab ?? "overview";
  const tabs = profileTabs(own).map((entry) => ({ id: entry.id, label: entry.label }));
  const onTab = (next: string) => track("profile_tab_changed", { tab: next as ProfileTab }, { sid, story: STORY });

  const navigation = {
    tabs: tabs.map((entry) => ({ ...entry, href: profileHref(address, entry.id) })),
    active: activeTab,
    onTab,
  };

  // Overview and Places have the profile site's layout: its own tab bar (scrollable on narrow
  // screens) and no section side panel, so they stay outside the context the Sites chrome turns
  // into that panel.
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

  return (
    <div className="landings-profile">
      <ProfileNavigationContext value={navigation}>
        <ActiveTab tab={activeTab} address={address} profile={profile} own={own} tabs={tabs} sid={sid} />
      </ProfileNavigationContext>
    </div>
  );
}

/** Each tab reads only its own data, so opening one tab does not fetch the others. */
function ActiveTab({ tab, ...props }: TabProps & { tab: ProfileTab }) {
  switch (tab) {
    case "creations":
      return <CreationsTab {...props} />;
    case "photos":
      return <PhotosTab {...props} />;
    case "communities":
      return <CommunitiesTab {...props} />;
    case "assets":
      return <AssetsTab {...props} />;
    case "referral-rewards":
    default:
      return <StProfileOverviewTab profile={props.profile as never} activeTab="overview" isOwnProfile={props.own} />;
  }
}

function CreationsTab({ address, profile, own, tabs }: TabProps) {
  const { data } = useProfileCreations(address, profile.name);
  return (
    <StProfileCreationsTab
      profile={profile as never}
      tabs={tabs}
      isOwnProfile={own}
      wearables={data?.wearables ?? []}
      emotes={data?.emotes ?? []}
    />
  );
}

function PhotosTab({ address, profile, own }: TabProps) {
  const { data } = useProfilePhotos(address);
  return (
    <StProfilePhotosTab
      profile={profile as never}
      isOwnProfile={own}
      photos={(data ?? []) as ComponentProps<typeof StProfilePhotosTab>["photos"]}
    />
  );
}

function AssetsTab({ address, profile, tabs }: TabProps) {
  const { data } = useProfileAssets(address);
  return (
    <StProfileMyAssetsTab
      profile={profile as never}
      tabs={tabs}
      wearables={data.wearables as ComponentProps<typeof StProfileMyAssetsTab>["wearables"]}
      names={data.names as ComponentProps<typeof StProfileMyAssetsTab>["names"]}
    />
  );
}

function CommunitiesTab({ address, profile, own, tabs, sid }: TabProps) {
  const { identity } = useAuth();
  const query = useMyCommunities(identity, own);
  const failed = own && query.isError;
  const reported = useRef(false);
  useEffect(() => {
    if (!failed || reported.current) return;
    reported.current = true;
    track("profile_communities_load_failed", { address }, { sid, story: STORY });
  }, [failed, address, sid]);

  return (
    <StProfileCommunitiesTab
      profile={profile as never}
      tabs={tabs}
      communities={(own ? (query.data ?? []) : []) as ComponentProps<typeof StProfileCommunitiesTab>["communities"]}
      isOwnProfile={own}
      loading={own && !!identity && query.isPending}
    />
  );
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
