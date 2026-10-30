// from: marketplace@3bf3662f:webapp/src/components/AssetFilters/{PriceFilter/{PriceFilter.tsx,utils.ts},
// Inventory/Inventory.tsx}, ui@8a63476:src/components/BarChart/{BarChart.tsx,BarChart.css,utils.ts},
// ui@8a63476:src/components/SliderField/{SliderField.tsx,SliderField.css}
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { ShopClient } from "../api/shop";
import { shopClient } from "../api/shop";
import type { LandFilters } from "./api";
import { fetchRentalListingPrices } from "./rentals";

export const CHART_LOG_SCALE = 5;
const CHART_BAR_QTY = 24;
const SLIDER_STEP = 0.1;
const LAND_UPPER_BOUND = 1_000_000_000;
const ACTIVE_BAR_COLOR = "#ff2d55";
const NON_ACTIVE_BAR_COLOR = "#4f1414";
const HOVERED_BAR_COLOR = "pink";
const LOADING_BAR_COLOR = "grey";

export const inverseScale = (n: number) => (n !== 0 ? Math.log(n) / Math.log(CHART_LOG_SCALE) : 0);

export function roundNumber(n: number, decimals = 2): number {
  if (n < 10) return Number(n.toFixed(decimals));
  const exp = n > 1000 ? 1000 : n > 100 ? 100 : 1;
  return Math.round(n / exp) * exp;
}

function createExponentialRange(min: number, max: number, upperBound?: number): number[] {
  const lo = inverseScale(min);
  const step = (inverseScale(max) - lo) / CHART_BAR_QTY;
  const arr: number[] = [];
  for (let i = 0; i < CHART_BAR_QTY; i++) arr.push(i === 0 ? lo : arr[i - 1]! + step);
  return [...arr.map((v) => (v !== 0 ? roundNumber(CHART_LOG_SCALE ** v) : v)), ...(upperBound && max > upperBound ? [upperBound] : [])];
}

export type ChartRange = { values: [number, number]; amount: number };

export function barChartRanges(data: Record<string, number>, min: number, max: number, upperBound?: number): ChartRange[] {
  const ranges = createExponentialRange(min, max, upperBound);
  const bars = new Map<number, number>(ranges.map((r) => [r, 0]));
  for (const [key, value] of Object.entries(data)) {
    const n = roundNumber(Number(key));
    const idx = ranges.findIndex((r) => n <= r);
    const at = ranges[idx > 0 ? idx - 1 : idx] ?? ranges[ranges.length - 1]!;
    bars.set(at, (bars.get(at) ?? 0) + value);
  }
  const keys = [...bars.keys()];
  return keys.map((k, i) => ({ values: [k, keys[i + 1] ?? k], amount: bars.get(k)! }));
}

const inRange = (r: readonly number[], min: number, max: number) => (r[0]! >= min || r[1]! > min) && (r[0]! < max || r[1]! <= max);

const COMPACT = new Intl.NumberFormat("en", { notation: "compact" });

function fromWei(data: Record<string, number>): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(data)) {
    let mana: number;
    try {
      mana = Number(BigInt(k) / 10n ** 12n) / 1e6;
    } catch {
      mana = Number(k) / 1e18;
    }
    out[String(mana)] = (out[String(mana)] ?? 0) + v;
  }
  return out;
}

export function landPricesQuery(f: LandFilters): Record<string, string | boolean> {
  const q: Record<string, string | boolean> = { category: f.category };
  if (f.adjacentToRoad) q.adjacentToRoad = true;
  if (f.minDistanceToPlaza) q.minDistanceToPlaza = f.minDistanceToPlaza;
  if (f.maxDistanceToPlaza) q.maxDistanceToPlaza = f.maxDistanceToPlaza;
  if (f.minEstateSize) q.minEstateSize = f.minEstateSize;
  if (f.maxEstateSize) q.maxEstateSize = f.maxEstateSize;
  return q;
}

async function fetchLandPrices(f: LandFilters, client: ShopClient | undefined, signal: AbortSignal): Promise<Record<string, number>> {
  if (f.status === "rent") {
    const data = await fetchRentalListingPrices(
      {
        ...(f.category !== "land" ? { category: f.category } : {}),
        ...(f.rentalDays.length ? { rentalDays: f.rentalDays } : {}),
        ...(f.minEstateSize ? { minEstateSize: Number(f.minEstateSize) } : {}),
        ...(f.maxEstateSize ? { maxEstateSize: Number(f.maxEstateSize) } : {}),
        ...(f.minDistanceToPlaza ? { minDistanceToPlaza: Number(f.minDistanceToPlaza) } : {}),
        ...(f.maxDistanceToPlaza ? { maxDistanceToPlaza: Number(f.maxDistanceToPlaza) } : {}),
        ...(f.adjacentToRoad ? { adjacentToRoad: true } : {}),
      } as never,
      { client, signal },
    );
    return fromWei(data ?? {});
  }
  const c = client ?? shopClient();
  const body = await c.market.prices<{ data?: Record<string, number> }>(landPricesQuery(f), { signal });
  return fromWei(body?.data ?? {});
}

type Props = {
  filters: LandFilters;
  client?: ShopClient;
  inputs: ReactNode;
  min: string;
  max: string;
  onChange(min: string, max: string): void;
};

export default function LandPriceChart({ filters, client, inputs, min, max, onChange }: Props) {
  const [data, setData] = useState<Record<string, number> | null>(null);
  const [loading, setLoading] = useState(false);
  const [hover, setHover] = useState<number | null>(null);
  const [value, setValue] = useState<[string, string]>([min, max]);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => setValue([min, max]), [min, max]);
  useEffect(() => () => void (timer.current && clearTimeout(timer.current)), []);

  const key = JSON.stringify([filters.status, filters.category, filters.rentalDays, landPricesQuery(filters)]);
  useEffect(() => {
    const ctrl = new AbortController();
    setLoading(true);
    fetchLandPrices(filters, client, ctrl.signal).then(
      (d) => {
        if (ctrl.signal.aborted) return;
        setData(d);
        setLoading(false);
      },
      () => {
        if (!ctrl.signal.aborted) setLoading(false);
      },
    );
    return () => ctrl.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, client]);

  const chart = useMemo(() => {
    if (!data) return null;
    const keys = Object.keys(data).map(Number).filter(Number.isFinite);
    if (!keys.length) return null;
    const dsMax = Math.max(...keys);
    const rangeMax = LAND_UPPER_BOUND < dsMax ? LAND_UPPER_BOUND : dsMax;
    const rangeMin = Math.min(...keys);
    if (!(rangeMax > 0)) return null;
    return { rangeMin, rangeMax, dsMax, ranges: barChartRanges(data, rangeMin, rangeMax, LAND_UPPER_BOUND) };
  }, [data]);

  if (!chart) {
    return (
      <div className="shop-lb-chart">
        {inputs}
        {loading ? <div className="shop-lb-chart__box shop-lb-chart__box--loading" aria-hidden="true" /> : null}
      </div>
    );
  }

  const { rangeMin, rangeMax, dsMax, ranges } = chart;
  const inMin = inverseScale(rangeMin);
  const inMax = inverseScale(rangeMax);
  const clamp = (v: number) => Math.max(inMin, Math.min(inMax, v));
  const from = clamp(value[0] ? inverseScale(Number(value[0])) : inMin);
  const to = Math.max(from, clamp(value[1] ? inverseScale(Number(value[1])) : inMax));
  const span = inMax - inMin || 1;
  const pct = (v: number) => ((v - inMin) / span) * 100;
  const lo = Number(value[0] || rangeMin);
  const hi = Number(value[1] || rangeMax);
  const peak = Math.max(1, ...ranges.map((r) => r.amount));

  const emit = (next: [string, string]) => {
    setValue(next);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => onChange(next[0], next[1]), 500);
  };
  const slide = (a: number, b: number) => {
    const isMax = inMax - b <= SLIDER_STEP;
    emit([String(roundNumber(CHART_LOG_SCALE ** a)), String(roundNumber(isMax ? rangeMax : CHART_LOG_SCALE ** b))]);
  };

  const W = 272;
  const H = 150;
  const PAD_X = 12;
  const PAD_TOP = 20;
  const PAD_BOTTOM = 5;
  const slot = (W - PAD_X * 2) / ranges.length;
  const maxLabel = `${COMPACT.format(Number(value[1] || rangeMax))}${!value[1] && LAND_UPPER_BOUND < dsMax ? "+" : ""}`;

  return (
    <div className="shop-lb-chart">
      {inputs}
      <div className="shop-lb-chart__box" onMouseLeave={() => setHover(null)}>
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" width="100%" height={H} role="img" aria-label="Price distribution">
          {ranges.map((r, i) => {
            const h = (r.amount / peak) * (H - PAD_TOP - PAD_BOTTOM);
            const fill = hover === i ? HOVERED_BAR_COLOR : loading ? LOADING_BAR_COLOR : inRange(r.values, lo, hi) ? ACTIVE_BAR_COLOR : NON_ACTIVE_BAR_COLOR;
            return (
              <g key={i} onMouseEnter={() => setHover(i)} onClick={() => emit(r.values[0] === r.values[1] ? [String(r.values[0]), ""] : [String(roundNumber(r.values[0])), String(roundNumber(r.values[1]))])}>
                <rect x={PAD_X + i * slot} y={PAD_TOP} width={slot} height={H - PAD_TOP - PAD_BOTTOM} fill="transparent" />
                <rect x={PAD_X + i * slot + (slot - 8) / 2} y={H - PAD_BOTTOM - h} width={8} height={h} fill={fill} />
              </g>
            );
          })}
        </svg>
        {hover !== null && ranges[hover] ? (
          <div className="shop-lb-chart__tip" role="tooltip">
            {COMPACT.format(ranges[hover]!.values[0])} - {COMPACT.format(ranges[hover]!.values[1])}: {ranges[hover]!.amount.toLocaleString("en-US")}
          </div>
        ) : null}
      </div>
      <div className="shop-lb-slider">
        <input
          type="range"
          className="shop-lb-slider__input shop-lb-slider__input--from"
          aria-label="Min price"
          aria-valuetext={COMPACT.format(lo)}
          min={inMin}
          max={inMax}
          step={SLIDER_STEP}
          value={from}
          onChange={(e) => slide(Math.min(Number(e.target.value), to), to)}
        />
        <input
          type="range"
          className="shop-lb-slider__input shop-lb-slider__input--to"
          aria-label="Max price"
          aria-valuetext={maxLabel}
          min={inMin}
          max={inMax}
          step={SLIDER_STEP}
          value={to}
          onChange={(e) => slide(from, Math.max(Number(e.target.value), from))}
        />
        <div className="shop-lb-slider__rail" aria-hidden="true">
          <div className="shop-lb-slider__track" style={{ left: `${pct(from)}%`, width: `${pct(to) - pct(from)}%` }} />
          <span className="shop-lb-slider__mark" style={{ left: `${pct(from)}%` }} />
          <span className="shop-lb-slider__mark" style={{ left: `${pct(to)}%` }} />
        </div>
      </div>
      {value[0] && value[1] && Number(value[1]) <= Number(value[0]) ? <span className="shop-lb-chart__error">The maximum price should be higher than the minimum</span> : null}
    </div>
  );
}
