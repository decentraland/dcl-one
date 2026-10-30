import { siteUrl } from "../../data/site";
import type { MouseEvent } from "react";
import { useState } from "react";
import { SitesChromeMaybe } from "../frames/SitesChrome";
import Spinner from "../../atoms/Spinner";
import Modal from "../../components/Modal";
import { type LabelSuffixProps } from "../../components/labelSuffix";
import "./stprofilecommunitiestab.css";

import ProfileTabLayout, { MEMBER_TABS, OWN_TABS, ProfileActions } from "../frames/ProfileTabLayout";

const GroupsGlyph = () => (
  <svg viewBox="0 0 24 24" width="1em" height="1em" aria-hidden="true" fill="currentColor">
    <path d="M12 12.75c1.63 0 3.07.39 4.24.9 1.08.48 1.76 1.56 1.76 2.73V18H6v-1.61c0-1.18.68-2.26 1.76-2.73 1.17-.52 2.61-.91 4.24-.91zM4 13c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm1.13 1.1c-.37-.06-.74-.1-1.13-.1-.99 0-1.93.21-2.78.58A2.01 2.01 0 0 0 0 16.43V18h4.5v-1.61c0-.83.23-1.61.63-2.29zM20 13c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm4 3.43c0-.81-.48-1.53-1.22-1.85A6.95 6.95 0 0 0 20 14c-.39 0-.76.04-1.13.1.4.68.63 1.46.63 2.29V18H24v-1.57zM12 6c1.66 0 3 1.34 3 3s-1.34 3-3 3-3-1.34-3-3 1.34-3 3-3z" />
  </svg>
);
const PeopleGlyph = () => (
  <svg viewBox="0 0 24 24" width="1em" height="1em" aria-hidden="true" fill="currentColor">
    <path d="M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z" />
  </svg>
);
const CheckGlyph = () => (
  <svg viewBox="0 0 24 24" width="1em" height="1em" aria-hidden="true" fill="currentColor">
    <path d="M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
  </svg>
);
const CopyGlyph = () => (
  <svg viewBox="0 0 24 24" width="1em" height="1em" aria-hidden="true" fill="currentColor">
    <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z" />
  </svg>
);
const CloseGlyph = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="currentColor">
    <path d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" />
  </svg>
);
const JumpInBadge = () => (
  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <rect x="0.75" y="0.75" width="22.5" height="22.5" rx="7.25" stroke="currentColor" strokeOpacity="0.7" strokeWidth="1.5" />
    <path
      d="M18.7111 11.065L14.034 6.39027C13.2002 5.55695 11.7971 6.14637 11.7971 7.32523V8.86994C11.7564 8.86994 11.7361 8.86994 11.6954 8.86994H7.25895C6.50654 8.86994 5.89648 9.45936 5.89648 10.2114V13.7683C5.89648 14.5203 6.50654 15.1301 7.25895 15.1301H11.6751C11.7158 15.1301 11.7361 15.1301 11.7768 15.1301V16.6748C11.7768 17.8536 13.2002 18.4431 14.0137 17.6097L18.6908 12.935C19.2195 12.4065 19.2195 11.5732 18.7111 11.065Z"
      fill="currentColor"
    />
  </svg>
);

type TabItem = { id: string; label: string };

export type Community = {
  id: string;
  name: string;
  membersCount: number;
  role: string;
  thumb: string | null;
};

export type Profile = {
  address: string;
  name?: string;
  hasClaimedName?: boolean;
  nameColor?: string;
  mutualCount: number;
};

const EMPTY_PROFILE: Profile = { address: "", mutualCount: 0 };

type CommunityCardProps = {
  community: Community;
  isOwnProfile: boolean;
  onOpen: (id: string) => void;
};

function CommunityCard({ community, isOwnProfile, onOpen }: CommunityCardProps) {
  const [copied, setCopied] = useState(false);
  const isOwner = community.role === "owner" || community.role === "admin";
  const roleLabel = community.role === "owner" ? "Owner" : "Admin";

  function handleShare(e: MouseEvent<HTMLButtonElement>) {
    e.preventDefault();
    e.stopPropagation();
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <a
      className="spc__card"
      href={`/landings/community-detail?id=${community.id}`}
      onClick={(e) => {
        e.preventDefault();
        onOpen(community.id);
      }}
    >
      <div className="spc__thumb">
        {community.thumb ? (
          <span className="spc__thumbimg" style={{ background: community.thumb }} aria-hidden="true" />
        ) : (
          <span className="spc__fallback" aria-hidden="true">
            <GroupsGlyph />
          </span>
        )}
        {isOwner ? <span className="spc__ownerchip">{roleLabel}</span> : null}
        {typeof community.membersCount === "number" ? (
          <span className="spc__membercount">
            <PeopleGlyph />
            {community.membersCount}
          </span>
        ) : null}
      </div>
      <div className="spc__cardbody">
        <span className="spc__name">{community.name}</span>
        <div className="spc__actionrow">
          <span className="spc__actionbtn">
            {isOwner || !isOwnProfile ? (
              "View"
            ) : (
              <>
                <CheckGlyph />
                Joined
              </>
            )}
          </span>
          <button
            type="button"
            className="spc__sharebtn"
            onClick={handleShare}
            aria-label="Copy link"
            title={copied ? "Copied!" : "Copy link"}
          >
            <CopyGlyph />
          </button>
        </div>
      </div>
    </a>
  );
}

type StProfileCommunitiesTabProps = LabelSuffixProps & {
  chrome?: boolean;
  profile?: Profile;
  tabs?: TabItem[];
  communities?: Community[];
  isOwnProfile?: boolean;
  loading?: boolean;
};

export default function StProfileCommunitiesTab({
  labelSuffix,
  chrome = true,
  profile = EMPTY_PROFILE,
  tabs,
  communities = [],
  isOwnProfile = false,
  loading = false,
}: StProfileCommunitiesTabProps) {
  const [openId, setOpenId] = useState<string | null>(null);
  const openCommunity = communities.find((c) => c.id === openId) || null;

  return (
    <SitesChromeMaybe chrome={chrome} active="legal">
      <ProfileTabLayout
        className="spc"
        labelSuffix={labelSuffix}
        profile={profile}
        tabs={tabs ?? (isOwnProfile ? OWN_TABS : MEMBER_TABS)}
        activeTab="communities"
        actions={<ProfileActions isOwnProfile={isOwnProfile} hasClaimedName={profile.hasClaimedName} mutualCount={profile.mutualCount} />}
      >
                <div className="spc__body">
                  {loading ? (
                    <div className="spc__loadingrow">
                      <Spinner size={28} />
                    </div>
                  ) : communities.length === 0 ? (
                    !isOwnProfile ? (
                      <p className="spc__emptymember">This member has not joined any communities yet.</p>
                    ) : (
                      <div className="spc__emptystate">
                        <span className="spc__emptyicon" aria-hidden="true">
                          <GroupsGlyph />
                        </span>
                        <div className="spc__emptybody">
                          <h2 className="spc__emptytitle">No communities yet</h2>
                          <p className="spc__emptysubtitle">
                            Jump into Decentraland and join the community that better aligns with you. You can also create your own!
                          </p>
                          <a className="spc__emptycta" href={siteUrl("/play/#/communities")}>
                            Explore communities
                            <span className="spc__emptyctaicon"><JumpInBadge /></span>
                          </a>
                        </div>
                      </div>
                    )
                  ) : (
                    <>
                      <p className="spc__countlabel">{communities.length} communities</p>
                      <div className="spc__grid">
                        {communities.map((community) => (
                          <CommunityCard
                            key={community.id}
                            community={community}
                            isOwnProfile={isOwnProfile}
                            onOpen={setOpenId}
                          />
                        ))}
                      </div>
                    </>
                  )}
                </div>
      </ProfileTabLayout>

      {openCommunity ? (
        <Modal onClose={() => setOpenId(null)} width="100%" className="modal__card--plain spc__dialog" ariaLabel={openCommunity.name}>
            <div className="spc__dialogheader">
              <button type="button" className="spc__dialogclose" aria-label="Close" onClick={() => setOpenId(null)}>
                <CloseGlyph />
              </button>
            </div>
            <div className="spc__dialogbanner" style={{ background: openCommunity.thumb || undefined }}>
              {!openCommunity.thumb ? (
                <span className="spc__dialogbannerfallback" aria-hidden="true"><GroupsGlyph /></span>
              ) : null}
            </div>
            <div className="spc__dialogbody">
              <h2 className="spc__dialogtitle">{openCommunity.name}</h2>
              <div className="spc__dialogmeta">
                <PeopleGlyph />
                {typeof openCommunity.membersCount === "number" ? `${openCommunity.membersCount} members` : "Members"}
              </div>
              <p className="spc__dialogdesc">
                A place for {openCommunity.name} to gather, share scenes and host events across Decentraland.
              </p>
              <button type="button" className="spc__dialogjoin">Join community</button>
            </div>
        </Modal>
      ) : null}
    </SitesChromeMaybe>
  );
}
