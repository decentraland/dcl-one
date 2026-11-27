export type CommunityPrivacy = "public" | "private";
export type CommunityVisibility = "all" | "unlisted";

export type OwnedPlace = {
  id: string;
  title: string;
  coords: string;
  kind: "land" | "world";
};

export type CommunityDraft = {
  name: string;
  description: string;
  privacy: CommunityPrivacy;
  visibility: CommunityVisibility;
  hasThumbnail: boolean;
  thumbnailPreviewUrl: string | null;
  placeIds: string[];
};

export function emptyDraft(): CommunityDraft {
  return {
    name: "",
    description: "",
    privacy: "public",
    visibility: "all",
    hasThumbnail: false,
    thumbnailPreviewUrl: null,
    placeIds: [],
  };
}

export type CreateResult = {
  id: string;
  name: string;
  privacy: CommunityPrivacy;
  visibility: CommunityVisibility;
};
