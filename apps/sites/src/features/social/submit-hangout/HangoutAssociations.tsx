import type { HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import FeaturedAssetPicker from "./FeaturedAssetPicker";

export default function HangoutAssociations({ draft, options, canEditAny, canChangeCommunity, onEdit }: {
  draft: HangoutDraft; options: HangoutOptions | null; canEditAny: boolean; canChangeCommunity: boolean;
  onEdit: (patch: Partial<HangoutDraft>) => void;
}) {
  const knownCommunity = options?.communities.some((community) => community.id === draft.communityId);
  const schedules = options?.schedules ?? [];
  const selectedSchedules = draft.scheduleIds ?? [];
  return <div className="submit-hangout__associations">
    <label className="submit-hangout__fieldlabel">Community
      <select aria-label="Community" className="submit-hangout__field" value={draft.communityId} disabled={!canChangeCommunity || !options || Boolean(options.communitiesError)} onChange={(event) => onEdit({ communityId: event.target.value })}>
        <option value="">None</option>
        {draft.communityId && !knownCommunity && <option value={draft.communityId}>Current community</option>}
        {options?.communities.map((community) => <option value={community.id} key={community.id}>{community.name}</option>)}
      </select>
    </label>
    {!options && <p role="status">Loading communities and schedules…</p>}
    {options?.communitiesError && <p role="alert">Could not load your communities: {options.communitiesError}</p>}
    {!canChangeCommunity && draft.communityId && <p className="submit-hangout__hint">Community owners and moderators can change this association.</p>}
    <FeaturedAssetPicker value={draft.featuredItem ?? ""} onChange={(featuredItem) => onEdit({ featuredItem })} />
    {canEditAny && <>
      <fieldset className="submit-hangout__assetpicker"><legend>Schedules</legend>
        {options?.schedulesError ? <p role="alert">Could not load schedules: {options.schedulesError}</p> : <>
          {schedules.length === 0 && options && <p className="submit-hangout__hint">No schedules are available.</p>}
          {schedules.map((schedule) => <label className="submit-hangout__fieldcheck" key={schedule.id}><input type="checkbox" checked={selectedSchedules.includes(schedule.id)} onChange={(event) => onEdit({ scheduleIds: event.target.checked ? [...selectedSchedules, schedule.id] : selectedSchedules.filter((id) => id !== schedule.id) })} />{schedule.name}</label>)}
          {selectedSchedules.filter((id) => !schedules.some((schedule) => schedule.id === id)).map((id) => <label className="submit-hangout__fieldcheck" key={id}><input type="checkbox" checked onChange={() => onEdit({ scheduleIds: selectedSchedules.filter((selected) => selected !== id) })} />Unavailable schedule (remove association)</label>)}
        </>}
      </fieldset>
      <label className="submit-hangout__fieldlabel">Custom join link<input type="url" className="submit-hangout__field" value={draft.joinUrl ?? ""} onChange={(event) => onEdit({ joinUrl: event.target.value })} placeholder="Use the venue's default link" /></label>
    </>}
  </div>;
}
