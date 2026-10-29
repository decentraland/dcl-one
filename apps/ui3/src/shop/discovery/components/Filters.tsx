// from: shop@2d3bf62:app/src/components/Filters/Filters.tsx (+ Filters.styles.ts),
//       app/src/components/CategoryFilter/CategoryFilter.tsx (+ CategoryFilter.styles.ts)

import { useEffect, useId, useState, type ReactNode } from "react";

import { t } from "../intl";
import { CATEGORIES, type Sub, type Top } from "../lib/categories";
import { PRICE_SLIDER_MAX, PRICE_SLIDER_STEPS, priceToSliderPct, priceToSliderPos, sliderPosToPrice } from "../lib/price-slider";
import { RARITIES, rarityColor, rarityLabel } from "../lib/rarity";
import Icon from "./Icon";
import "./filters.css";

export type FilterStatus = "all" | "on_sale" | "not_for_sale";
export const STATUSES: FilterStatus[] = ["all", "on_sale", "not_for_sale"];

function parentOfLeaf(key: string | null): string | null {
  for (const top of CATEGORIES) for (const sub of top.subs ?? []) if (sub.subs?.some((leaf) => leaf.key === key)) return sub.key;
  return null;
}

export function subCategoryLabel(key: string): string {
  for (const top of CATEGORIES)
    for (const sub of top.subs ?? []) {
      if (sub.key === key) return t(sub.labelKey);
      const leaf = sub.subs?.find((l) => l.key === key);
      if (leaf) return t(leaf.labelKey);
    }
  return key;
}

export function CategoryFilter({
  category,
  subCategory,
  onCategory,
  onSub,
  hideNames = false,
  hideAll = false,
  collections = false,
  onCollections,
}: {
  category: string;
  subCategory: string | null;
  onCategory: (key: string) => void;
  onSub: (key: string | null) => void;
  hideNames?: boolean;
  hideAll?: boolean;
  collections?: boolean;
  onCollections?: () => void;
}) {
  const [expandedKey, setExpandedKey] = useState<string | null>(() => (CATEGORIES.some((top) => top.key === category && top.subs) ? category : null));
  const [expandedSubKey, setExpandedSubKey] = useState<string | null>(() => parentOfLeaf(subCategory));

  useEffect(() => {
    if (CATEGORIES.some((top) => top.key === category && top.subs)) setExpandedKey(category);
    const parent = parentOfLeaf(subCategory);
    if (parent) setExpandedSubKey(parent);
  }, [category, subCategory]);

  const clickSub = (sub: Sub) => {
    onSub(subCategory === sub.key ? null : sub.key);
    if (sub.subs) setExpandedSubKey((prev) => (prev === sub.key ? null : sub.key));
  };
  const clickTop = (top: Top) => {
    if (top.subs) {
      setExpandedKey((prev) => {
        const willOpen = prev !== top.key;
        if (willOpen) onCategory(top.key);
        return willOpen ? top.key : null;
      });
    } else {
      onCategory(top.key);
      setExpandedKey(null);
    }
  };

  return (
    <div className="shop-cats" data-testid="category-filter">
      {CATEGORIES.filter((top) => !(hideAll && top.key === "all") && !(hideNames && top.key === "names")).map((top) => {
        const open = expandedKey === top.key && !!top.subs;
        const selected = !collections && top.key === category && !subCategory;
        return (
          <div key={top.key} className="shop-cats__group">
            <button
              type="button"
              className="shop-cats__cat"
              data-selected={selected || undefined}
              data-expanded={open || undefined}
              aria-expanded={top.subs ? open : undefined}
              aria-pressed={selected}
              onClick={() => clickTop(top)}
            >
              <span>{t(top.labelKey)}</span>
              {top.expandable ? <Icon name="chevron-down" size={24} className={open ? "shop-cats__chev shop-cats__chev--open" : "shop-cats__chev"} /> : null}
            </button>
            {top.subs ? (
              <div className="shop-cats__subs" data-open={open || undefined} inert={!open || undefined}>
                <div className="shop-cats__subsinner">
                  {top.subs.map((sub) => {
                    const subOpen = expandedSubKey === sub.key && !!sub.subs;
                    return (
                      <div key={sub.key}>
                        <button type="button" className="shop-cats__sub" data-active={subCategory === sub.key || undefined} aria-pressed={subCategory === sub.key} onClick={() => clickSub(sub)}>
                          <span className="shop-cats__left">
                            <Icon name={sub.icon} size={24} />
                            <span className="shop-cats__label">{t(sub.labelKey)}</span>
                          </span>
                          {sub.expandable ? <Icon name="chevron-down" size={24} className={subOpen ? "shop-cats__chev shop-cats__chev--open" : "shop-cats__chev"} /> : null}
                        </button>
                        {sub.subs ? (
                          <div className="shop-cats__subs" data-open={subOpen || undefined} inert={!subOpen || undefined}>
                            <div className="shop-cats__subsinner">
                              {sub.subs.map((leaf) => (
                                <button
                                  key={leaf.key}
                                  type="button"
                                  className="shop-cats__sub shop-cats__sub--leaf"
                                  data-active={subCategory === leaf.key || undefined}
                                  aria-pressed={subCategory === leaf.key}
                                  onClick={() => onSub(subCategory === leaf.key ? null : leaf.key)}
                                >
                                  <span className="shop-cats__left">
                                    <Icon name={leaf.icon} size={24} />
                                    <span className="shop-cats__label">{t(leaf.labelKey)}</span>
                                  </span>
                                </button>
                              ))}
                            </div>
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : null}
          </div>
        );
      })}
      {onCollections ? (
        <div className="shop-cats__group">
          <button type="button" className="shop-cats__cat" data-testid="filter-collections" data-selected={collections || undefined} aria-pressed={collections} onClick={onCollections}>
            <span>{t("categories.collections")}</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}

export function FilterSection({
  title,
  open,
  onToggle,
  summary,
  desktopStatic,
  testId,
  children,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  summary?: string;
  desktopStatic?: boolean;
  testId?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="shop-filters__section" data-static={desktopStatic || undefined}>
      <button type="button" className="shop-filters__head" aria-expanded={open} aria-controls={id} onClick={onToggle} data-testid={testId}>
        <span className="shop-filters__title">{title}</span>
        <Icon name="chevron-down" size={24} className={open ? "shop-cats__chev shop-cats__chev--open" : "shop-cats__chev"} />
      </button>
      {!open && summary ? <p className="shop-filters__summary">{summary}</p> : null}
      <div id={id} className="shop-filters__collapse" data-open={open || undefined}>
        <div className="shop-filters__inner">{children}</div>
      </div>
    </section>
  );
}

export type FiltersProps = {
  category: string;
  subCategory: string | null;
  onCategory: (key: string) => void;
  onSub: (key: string | null) => void;
  priceMin: string;
  priceMax: string;
  onPriceMin: (v: string) => void;
  onPriceMax: (v: string) => void;
  rarities: string[];
  onToggleRarity: (r: string) => void;
  bodyShape?: string;
  onBodyShape?: (v: string) => void;
  listingType?: string;
  onListingType?: (v: string) => void;
  status: FilterStatus;
  onStatus: (s: FilterStatus) => void;
  smart: boolean;
  onSmart: (v: boolean) => void;
  deals?: boolean;
  onDeals?: (v: boolean) => void;
  hideNames?: boolean;
  hideAll?: boolean;
  hideStatus?: boolean;
  collections?: boolean;
  onCollections?: () => void;
};

export default function Filters(props: FiltersProps) {
  const { category, subCategory, priceMin, priceMax, rarities, status, smart, deals } = props;
  const [openCategory, setOpenCategory] = useState(true);
  const [openPrice, setOpenPrice] = useState(true);
  const [openRarity, setOpenRarity] = useState(false);
  const [openStatus, setOpenStatus] = useState(true);
  const statusName = useId();

  const min = priceMin && !Number.isNaN(Number(priceMin)) ? Number(priceMin) : undefined;
  const max = priceMax && !Number.isNaN(Number(priceMax)) ? Number(priceMax) : undefined;
  const [drag, setDrag] = useState<{ min?: number; max?: number }>({});
  const committedMin = min != null ? Math.min(min, PRICE_SLIDER_MAX) : 0;
  const committedMax = max != null ? Math.min(max, PRICE_SLIDER_MAX) : PRICE_SLIDER_MAX;
  const sliderMin = drag.min ?? committedMin;
  const sliderMax = drag.max ?? committedMax;
  const minText = (n: number) => (n <= 0 ? "" : String(n));
  const maxText = (n: number) => (n >= PRICE_SLIDER_MAX ? "" : String(n));
  const commitSlide = () => {
    if (drag.min === undefined && drag.max === undefined) return;
    if (drag.min !== undefined) props.onPriceMin(minText(drag.min));
    if (drag.max !== undefined) props.onPriceMax(maxText(drag.max));
    setDrag({});
  };

  const categorySummary = props.collections
    ? t("categories.collections")
    : subCategory
      ? subCategoryLabel(subCategory)
      : category !== "all"
        ? t(CATEGORIES.find((c) => c.key === category)?.labelKey ?? "")
        : "";
  const priceSummary = min != null || max != null ? `${priceMin || "0"}-${priceMax || "∞"}` : "";
  const raritySummary = RARITIES.filter((r) => rarities.includes(r)).map(rarityLabel).join(", ");
  const statusSummary = status === "on_sale" ? t("filter.onSale") : status === "not_for_sale" ? t("filter.notForSale") : "";
  const minPct = priceToSliderPct(sliderMin);
  const maxPct = priceToSliderPct(sliderMax);

  return (
    <div className="shop-filters" data-testid="filters">
      <FilterSection title={t("assets.category")} open={openCategory} onToggle={() => setOpenCategory((o) => !o)} summary={categorySummary} desktopStatic>
        <CategoryFilter
          category={category}
          subCategory={subCategory}
          onCategory={props.onCategory}
          onSub={props.onSub}
          hideNames={props.hideNames}
          hideAll={props.hideAll}
          collections={props.collections}
          onCollections={props.onCollections}
        />
      </FilterSection>

      <hr className="shop-filters__divider" />

      <FilterSection title={t("filter.price")} open={openPrice} onToggle={() => setOpenPrice((o) => !o)} summary={priceSummary} desktopStatic>
        <div className="shop-filters__prices">
          <label>
            <span>{t("assets.min")}</span>
            <span className="shop-filters__pricebox">
              <input type="number" min="0" inputMode="numeric" aria-label={t("assets.minPriceAria")} placeholder="0" value={drag.min !== undefined ? minText(drag.min) : priceMin} onChange={(e) => props.onPriceMin(e.target.value)} />
            </span>
          </label>
          <span className="shop-filters__to">{t("assets.priceTo")}</span>
          <label>
            <span>{t("assets.max")}</span>
            <span className="shop-filters__pricebox">
              <input type="number" min="0" inputMode="numeric" aria-label={t("assets.maxPriceAria")} placeholder="0" value={drag.max !== undefined ? maxText(drag.max) : priceMax} onChange={(e) => props.onPriceMax(e.target.value)} />
            </span>
          </label>
        </div>
        <div className="shop-filters__slider" style={{ ["--min-pct" as string]: `${minPct}%`, ["--max-pct" as string]: `${maxPct}%` }}>
          <span className="shop-filters__track" aria-hidden="true" />
          <span className="shop-filters__fill" aria-hidden="true" />
          <input
            type="range"
            min={0}
            max={PRICE_SLIDER_STEPS}
            value={priceToSliderPos(sliderMin)}
            aria-label={t("assets.minPriceSliderAria")}
            aria-valuetext={sliderMin.toLocaleString("en-US")}
            data-on-top={minPct >= 50 || undefined}
            onChange={(e) => setDrag((d) => ({ ...d, min: Math.min(sliderPosToPrice(Number(e.target.value)), d.max ?? committedMax) }))}
            onPointerUp={commitSlide}
            onKeyUp={commitSlide}
            onBlur={commitSlide}
          />
          <input
            type="range"
            min={0}
            max={PRICE_SLIDER_STEPS}
            value={priceToSliderPos(sliderMax)}
            aria-label={t("assets.maxPriceSliderAria")}
            aria-valuetext={sliderMax.toLocaleString("en-US")}
            onChange={(e) => setDrag((d) => ({ ...d, max: Math.max(sliderPosToPrice(Number(e.target.value)), d.min ?? committedMin) }))}
            onPointerUp={commitSlide}
            onKeyUp={commitSlide}
            onBlur={commitSlide}
          />
        </div>
        <div className="shop-filters__range" data-testid="price-range-labels">
          <span>
            <Icon name="credits" size={16} />
            {sliderMin.toLocaleString("en-US")}
          </span>
          <span>
            <Icon name="credits" size={16} />
            {sliderMax.toLocaleString("en-US")}
          </span>
        </div>
      </FilterSection>

      <hr className="shop-filters__divider" />

      <FilterSection title={t("assets.rarity")} open={openRarity} onToggle={() => setOpenRarity((o) => !o)} summary={raritySummary} testId="sidebar-section-toggle">
        <div className="shop-filters__rarities" data-testid="rarity-filter">
          {RARITIES.map((r) => {
            const selected = rarities.includes(r);
            return (
              <button key={r} type="button" className="shop-filters__rarity" aria-pressed={selected} onClick={() => props.onToggleRarity(r)} data-testid="rarity-filter-check">
                <span className="shop-filters__swatch" style={{ background: rarityColor(r) }}>
                  {selected ? <Icon name="check" size={16} /> : null}
                </span>
                <span>{rarityLabel(r)}</span>
              </button>
            );
          })}
        </div>
      </FilterSection>

      <hr className="shop-filters__divider" />

      {props.hideStatus ? null : (
        <>
          <FilterSection title={t("filter.status")} open={openStatus} onToggle={() => setOpenStatus((o) => !o)} summary={statusSummary}>
            <div role="radiogroup" aria-label={t("filter.status")}>
              {(
                [
                  ["all", t("filter.statusAll")],
                  ["on_sale", t("filter.onSale")],
                  ["not_for_sale", t("filter.notForSale")],
                ] as [FilterStatus, string][]
              ).map(([value, label]) => (
                <label key={value} className="shop-filters__radio">
                  <input type="radio" name={statusName} checked={status === value} onChange={() => props.onStatus(value)} />
                  <span>{label}</span>
                </label>
              ))}
            </div>
          </FilterSection>
          <hr className="shop-filters__divider" />
        </>
      )}

      {props.onBodyShape ? (
        <label className="shop-filters__toggle shop-filters__toggle--select">
          <span className="shop-filters__toggletitle">{t("itemDetail.bodyShape")}</span>
          <select aria-label={t("itemDetail.bodyShape")} value={props.bodyShape} onChange={(e) => props.onBodyShape?.(e.target.value)}>
            <option value="all">{t("filter.statusAll")}</option>
            <option value="male">{t("itemDetail.genderMale")}</option>
            <option value="female">{t("itemDetail.genderFemale")}</option>
            <option value="unisex">{t("itemDetail.genderUnisex")}</option>
          </select>
        </label>
      ) : null}
      {props.onListingType ? (
        <label className="shop-filters__toggle shop-filters__toggle--select">
          <span className="shop-filters__toggletitle">{t("filter.saleType")}</span>
          <select aria-label={t("filter.saleType")} value={props.listingType} onChange={(e) => props.onListingType?.(e.target.value)}>
            <option value="all">{t("filter.statusAll")}</option>
            <option value="primary">{t("filter.primary")}</option>
            <option value="secondary">{t("filter.secondary")}</option>
          </select>
        </label>
      ) : null}

      {props.onDeals ? (
        <div className="shop-filters__toggle" data-testid="deals-row">
          <span className="shop-filters__toggleleft">
            <span className="shop-filters__toggletitle">{t("filter.deals")}</span>
          </span>
          <button type="button" role="switch" className="shop-filters__switch" aria-checked={!!deals} aria-label={t("filter.deals")} data-testid="deals-toggle" onClick={() => props.onDeals?.(!deals)}>
            <span className="shop-filters__knob" />
          </button>
        </div>
      ) : null}
      <div className="shop-filters__toggle">
        <span className="shop-filters__toggleleft">
          <Icon name="smart" size={14} className="shop-filters__smart" />
          <span className="shop-filters__toggletitle">{t("filter.smart")}</span>
          <span className="shop-filters__info" role="img" tabIndex={0} aria-label={t("filter.smartHint")} title={t("filter.smartHint")} data-testid="smart-hint">
            <Icon name="info" size={12} />
          </span>
        </span>
        <button type="button" role="switch" className="shop-filters__switch" aria-checked={smart} aria-label={t("filter.smart")} onClick={() => props.onSmart(!smart)}>
          <span className="shop-filters__knob" />
        </button>
      </div>
    </div>
  );
}
