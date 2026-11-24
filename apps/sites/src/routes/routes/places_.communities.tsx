import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useDeferredValue, useState } from "react";
import { Link } from "react-router";
import type { CommunityCardLinkProps } from "@dcl/one-ui";
import { CommunitiesDirectory, type CommunitiesDirectoryLabels } from "@dcl/one-ui/apps/sites/places/CommunitiesDirectory";
import { PlacesPage } from "@dcl/one-ui/apps/sites/places/PlacesPage";

import { usePageView } from "@/core/lib/telemetry/page-view";
import { fetchCommunityDirectory } from "@/data/lib/catalyst/social/communities";
import SitesLayout from "@/features/components/chrome/SitesLayout";
import { COMMUNITIES_PAGE_VIEW } from "@/features/places/page-view";
import { communityHref, thumbnailUrl } from "@/features/social/community/view";

// decentraland.org's Places Communities tab (decentraland/sites `pages/places/DiscoverCommunitiesPage.tsx`), with its
// English copy (`discover.communities.*`).
const LABELS: CommunitiesDirectoryLabels = {
  heading: "Communities",
  search: "Search communities",
  searchPlaceholder: "Search communities",
  empty: "No communities found.",
  error: "Something went wrong loading communities.",
  retry: "Retry",
  loading: "Loading communities",
};

const count = new Intl.NumberFormat("en-US");

/** `{count, plural, one {# member} other {# members}}`. */
function membersLabel(members: number): string {
  return `${count.format(members)} ${members === 1 ? "member" : "members"}`;
}

function CardLink({ href, ...props }: CommunityCardLinkProps) {
  return <Link to={href} {...props} />;
}

export const meta = () => [{ title: "Communities | Decentraland" }];

export default function PlacesCommunities() {
  // `/places/*` owns its page views (the root sends none there), as the legacy tab did.
  usePageView(COMMUNITIES_PAGE_VIEW);
  const [search, setSearch] = useState("");
  const term = useDeferredValue(search.trim());
  const directory = useQuery({
    queryKey: ["communities", "directory", term],
    queryFn: ({ signal }) => fetchCommunityDirectory(term, { signal }),
    placeholderData: keepPreviousData,
  });
  const cards = (directory.data ?? []).map((community) => ({
    id: community.id,
    name: community.name,
    href: communityHref(community.id),
    image: thumbnailUrl(community),
    ownerName: community.ownerName,
    privacy: community.privacy,
    membersLabel: membersLabel(community.membersCount ?? 0),
    description: community.description,
  }));

  return (
    <SitesLayout active="explore">
      <PlacesPage title="Places">
        <CommunitiesDirectory
          search={search}
          onSearch={setSearch}
          state={directory.isError ? "error" : directory.isPending ? "loading" : "ready"}
          cards={cards}
          onRetry={() => void directory.refetch()}
          labels={LABELS}
          linkComponent={CardLink}
        />
      </PlacesPage>
    </SitesLayout>
  );
}
