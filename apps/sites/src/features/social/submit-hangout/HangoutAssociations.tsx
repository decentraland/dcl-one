import { useEventsCopy, Notice, WebSelect } from "@dcl/one-ui";
import type { HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import styles from "./HangoutAssociations.module.css";
import form from "./SubmitHangout.module.css";
import FeaturedAssetPicker from "./FeaturedAssetPicker";

export default function HangoutAssociations({ draft, options, canChangeCommunity, onEdit }: {
  draft: HangoutDraft; options: HangoutOptions | null; canChangeCommunity: boolean;
  onEdit: (patch: Partial<HangoutDraft>) => void;
}) {
  const { text } = useEventsCopy();
  const knownCommunity = options?.communities.some((community) => community.id === draft.communityId);
  return <div className={`${styles.root} ${form.associations}`}>
    <WebSelect label={text("Community")} value={draft.communityId} disabled={!canChangeCommunity} onChange={(event) => onEdit({ communityId: event.target.value })}>
      <option value="">{text("None")}</option>
      {draft.communityId && !knownCommunity && <option value={draft.communityId}>{text("Current community")}</option>}
      {options?.communities.map((community) => <option value={community.id} key={community.id}>{community.name}</option>)}
    </WebSelect>
    {!options && <p role="status">{text("Loading communities…")}</p>}
    {options?.communitiesError && <Notice tone="error">{text("Could not load your communities: {error}", { error: options.communitiesError })}</Notice>}
    {!canChangeCommunity && draft.communityId && <p className="submit-hangout__hint">{text("Community owners and moderators can change this association.")}</p>}
    <FeaturedAssetPicker value={draft.featuredItem ?? ""} onChange={(featuredItem) => onEdit({ featuredItem })} />
  </div>;
}
