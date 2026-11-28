import { useEventsCopy, Notice, WebSelect } from "@dcl/one-ui";
import type { HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import styles from "./HangoutAssociations.module.css";
import form from "./SubmitHangout.module.css";
import FeaturedAssetPicker from "./FeaturedAssetPicker";
import FormField from "./FormField";

// The native picker displays in-world names as plain text while keeping ordinary angle brackets.
const communityNameFormatting = /<\/?(?:size|color|b|i|u|s|mark|alpha|sub|sup)(?:=[^<>]*)?>|<#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})>/gi;

export default function HangoutAssociations({ draft, options, canChangeCommunity, onEdit }: {
  draft: HangoutDraft; options: HangoutOptions | null; canChangeCommunity: boolean;
  onEdit: (patch: Partial<HangoutDraft>) => void;
}) {
  const { text } = useEventsCopy();
  const knownCommunity = options?.communities.some((community) => community.id === draft.communityId);
  return <div className={`${styles.root} ${form.associations}`}>
    <FormField label={text("Community")}>
      {(id) => <WebSelect id={id} value={draft.communityId} disabled={!canChangeCommunity} onChange={(event) => onEdit({ communityId: event.target.value })}>
        <option value="">{text("None")}</option>
        {draft.communityId && !knownCommunity && <option value={draft.communityId}>{text("Current community")}</option>}
        {options?.communities.map((community) => (
          <option value={community.id} key={community.id}>
            {community.name.replace(communityNameFormatting, "").trim() || text("Untitled community")}
          </option>
        ))}
      </WebSelect>}
    </FormField>
    {!options && <p role="status">{text("Loading communities…")}</p>}
    {options?.communitiesError && <Notice tone="error">{text("Could not load your communities: {error}", { error: options.communitiesError })}</Notice>}
    {!canChangeCommunity && draft.communityId && <p className="submit-hangout__hint">{text("Community owners and moderators can change this association.")}</p>}
    <FeaturedAssetPicker value={draft.featuredItem ?? ""} onChange={(featuredItem) => onEdit({ featuredItem })} />
  </div>;
}
