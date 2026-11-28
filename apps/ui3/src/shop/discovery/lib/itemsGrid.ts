import { t } from "@shop/discovery/intl";
import { EMOTE_FILTER_DEFAULTS, emoteAttributes, type EmoteFilterState } from "@shop/discovery/lib/itemsGrid";
import type { FilterChip } from "../components/FilterBar";

export {
  dealsRequestedIn,
  defaultSortFor,
  defaultStatusFor,
  EMOTE_FILTER_DEFAULTS,
  emoteAttributes,
  ITEMS_PAGE_SIZE,
  itemsFilterDefaults,
  itemsGrid,
  itemsQuery,
  type EmoteFilterState,
  type ItemsFilterState,
  type ItemsGridScope,
} from "@shop/discovery/lib/itemsGrid";

/**
 * A browse page's emote filters: the sidebar props (offered only while browsing emotes), the active
 * chips, and the state a category change resets.
 */
export function emoteFilterBindings(category: string, state: EmoteFilterState, set: (patch: Partial<EmoteFilterState>) => void) {
  const emote = emoteAttributes(category, state);
  const browsing = category === "emote";
  const props = {
    emotePlayMode: state.emotePlayMode,
    onEmotePlayMode: browsing ? (v: string) => set({ emotePlayMode: v }) : undefined,
    emoteSound: state.emoteSound,
    onEmoteSound: browsing ? (v: boolean) => set({ emoteSound: v }) : undefined,
    emoteProps: state.emoteProps,
    onEmoteProps: browsing ? (v: boolean) => set({ emoteProps: v }) : undefined,
  };
  const chips: FilterChip[] = [];
  if (emote.emotePlayMode) chips.push({ key: "emotePlayMode", label: t(emote.emotePlayMode === "loop" ? "filter.emoteLoop" : "filter.emotePlayOnce"), onRemove: () => set({ emotePlayMode: "all" }) });
  if (emote.emoteHasSound) chips.push({ key: "emoteSound", label: t("filter.emoteWithSound"), onRemove: () => set({ emoteSound: false }) });
  if (emote.emoteHasGeometry) chips.push({ key: "emoteProps", label: t("filter.emoteWithProps"), onRemove: () => set({ emoteProps: false }) });
  const resetFor = (nextCategory: string): Partial<EmoteFilterState> => (nextCategory === "emote" ? {} : EMOTE_FILTER_DEFAULTS);
  return { emote, props, chips, resetFor };
}
