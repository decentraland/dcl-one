import { useId, useState, type PointerEvent, type ReactNode } from "react";

import {
  ChartDataTable,
  ChartGrid,
  ChartTooltip,
  ChartTooltipTable,
  DEFAULT_CHART_HEIGHT,
  DEFAULT_CHART_MARGIN,
  XTick,
  YTick,
  chartArea,
  cx,
  useChartWidth,
  type ChartMargin,
  type ChartPointer,
} from "./ChartFrame";
import { bandScale, defaultTickCount, linearScale, linearTicks, niceLinear } from "./chartScale";
import styles from "./chart.module.css";

export type StackedBarSeries = {
  id: string;
  label: string;
  color: string;
  /** One value for each date; `null` counts as 0 in the stack and reads "-" in the tooltip. */
  data: Array<number | null>;
};

export type StackedBarHover = { index: number; date: number; series: StackedBarSeries[] };

export type StackedBarChartProps = {
  /** Names the chart. The same text is the caption of the data table. */
  "aria-label": string;
  /** Epoch milliseconds, one for each bar. */
  dates: number[];
  /** Stacked in this order, the first at the bottom. */
  series: StackedBarSeries[];
  height?: number;
  /** Fixed width in pixels. Without it the chart takes the width of its container. */
  width?: number;
  margin?: ChartMargin;
  /** Part of each slot that stays empty (0 to 1). */
  gapRatio?: number;
  /** Corner radius of the top of each bar. */
  radius?: number;
  yTickCount?: number;
  /** Which bars take a label on the x axis. Default: every bar. */
  xTickFilter?: (date: number, index: number) => boolean;
  formatX?: (date: number) => string;
  formatY?: (value: number) => string;
  /** Date text of the tooltip title and of the data table. Default: `formatX`. */
  formatDate?: (date: number) => string;
  /** Content of the hover tooltip. Default: a table with one row for each series. */
  renderTooltip?: (hover: StackedBarHover) => ReactNode;
  className?: string;
  tooltipClassName?: string;
};

const defaultFormatX = (date: number) => new Date(date).toLocaleDateString();
const defaultFormatY = (value: number) => String(value);

const round3 = (value: number) => Math.round(value * 1000) / 1000;

/** A rectangle with round corners at the top (or at the bottom for a stack under zero). */
function stackOutline(x: number, top: number, width: number, height: number, radius: number, down: boolean) {
  const r = Math.max(0, Math.min(radius, height, width / 2));
  const bottom = top + height;
  const right = x + width;
  const n = round3;
  if (down) {
    return `M${n(x)},${n(top)}H${n(right)}V${n(bottom - r)}A${r},${r} 0 0 1 ${n(right - r)},${n(bottom)}H${n(x + r)}A${r},${r} 0 0 1 ${n(x)},${n(bottom - r)}Z`;
  }
  return `M${n(x)},${n(bottom)}V${n(top + r)}A${r},${r} 0 0 1 ${n(x + r)},${n(top)}H${n(right - r)}A${r},${r} 0 0 1 ${n(right)},${n(top + r)}V${n(bottom)}Z`;
}

/**
 * One bar for each date, split into a segment for each series. The whole slot of a bar opens one tooltip
 * that lists every series, so the pointer does not have to hit a segment.
 */
export default function StackedBarChart({
  "aria-label": ariaLabel,
  dates,
  series,
  height = DEFAULT_CHART_HEIGHT,
  width: fixedWidth,
  margin = DEFAULT_CHART_MARGIN,
  gapRatio = 0.2,
  radius = 0,
  yTickCount,
  xTickFilter,
  formatX = defaultFormatX,
  formatY = defaultFormatY,
  formatDate = formatX,
  renderTooltip,
  className,
  tooltipClassName,
}: StackedBarChartProps) {
  const id = "chart-" + useId().replace(/:/g, "");
  const [ref, width] = useChartWidth(fixedWidth);
  const [hovered, setHover] = useState<{ index: number; pointer: ChartPointer } | null>(null);
  const hoverDate = hovered ? dates[hovered.index] : undefined;
  const hover = hovered && hoverDate !== undefined ? { ...hovered, date: hoverDate } : null;

  const area = chartArea(width, height, margin);
  const ready = area.width > 0 && area.height > 0 && dates.length > 0;
  const right = area.left + area.width;
  const bottom = area.top + area.height;

  // Values over zero stack up and values under zero stack down, each from the zero line.
  const stacks = dates.map((_date, index) => {
    let up = 0;
    let down = 0;
    const segments = series.map(entry => {
      const value = entry.data[index] ?? 0;
      const from = value < 0 ? down : up;
      if (value < 0) down += value;
      else up += value;
      return { id: entry.id, color: entry.color, from, to: from + value };
    });
    return { segments, up, down };
  });
  const dataMin = Math.min(0, ...stacks.map(stack => stack.down));
  const dataMax = Math.max(0, ...stacks.map(stack => stack.up));
  const yCount = defaultTickCount(area.height, yTickCount);
  const [y0, y1] = niceLinear(dataMin, dataMax, yCount);
  const y = linearScale(y0, y1, bottom, area.top);
  const yTickValues = linearTicks(y0, y1, yCount);
  const band = bandScale(dates.length, area.left, right, gapRatio);
  const slotStart = (index: number) => band.start(index) - (band.step - band.bandwidth) / 2;

  const onPointerMove = (event: PointerEvent<SVGSVGElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    const px = event.clientX - box.left;
    const py = event.clientY - box.top;
    const index = Math.floor((px - area.left) / band.step);
    if (!ready || px < area.left || px > right || py < area.top || py > bottom || index < 0 || index >= dates.length) {
      setHover(null);
      return;
    }
    setHover({ index, pointer: { x: event.clientX, y: event.clientY } });
  };

  const cell = (entry: StackedBarSeries, index: number) => {
    const value = entry.data[index] ?? null;
    return value === null ? "-" : formatY(value);
  };

  const tooltip =
    hover &&
    (renderTooltip ? (
      renderTooltip({ index: hover.index, date: hover.date, series })
    ) : (
      <ChartTooltipTable
        title={formatDate(hover.date)}
        rows={series.map(entry => ({
          id: entry.id,
          label: entry.label,
          color: entry.color,
          value: cell(entry, hover.index),
        }))}
      />
    ));

  return (
    <div ref={ref} className={cx(styles.root, className)} style={{ height }}>
      <svg
        role="img"
        aria-label={ariaLabel}
        className={styles.svg}
        width={width}
        height={height}
        viewBox={"0 0 " + width + " " + height}
        onPointerMove={onPointerMove}
        onPointerDown={onPointerMove}
        onPointerLeave={() => setHover(null)}
      >
        {ready && (
          <>
            <ChartGrid area={area} ys={yTickValues.map(y)} />
            <g clipPath={"url(#" + id + "-plot)"}>
              {stacks.map((stack, index) => {
                const top = y(stack.up);
                const low = y(stack.down);
                return (
                  <clipPath key={index} id={id + "-stack-" + index}>
                    <path d={stackOutline(band.start(index), top, band.bandwidth, low - top, radius, stack.up === 0 && stack.down < 0)} />
                  </clipPath>
                );
              })}
              {series.map((entry, order) =>
                stacks.map((stack, index) => {
                  const segment = stack.segments[order];
                  if (!segment) return null;
                  const top = Math.min(y(segment.from), y(segment.to));
                  return (
                    <g key={entry.id + "-" + index} clipPath={"url(#" + id + "-stack-" + index + ")"}>
                      <rect
                        className={styles.bar}
                        data-series={entry.id}
                        x={round3(band.start(index))}
                        y={round3(top)}
                        width={round3(band.bandwidth)}
                        height={round3(Math.abs(y(segment.from) - y(segment.to)))}
                        fill={segment.color}
                      />
                    </g>
                  );
                }),
              )}
              {hover && (
                <rect className={styles.band} x={slotStart(hover.index)} y={area.top} width={band.step} height={area.height} />
              )}
            </g>
            <g transform={"translate(0, " + bottom + ")"}>
              <line className={styles.axisLine} x1={area.left} x2={right} />
              {dates.map((date, index) =>
                !xTickFilter || xTickFilter(date, index) ? (
                  <XTick key={date} at={slotStart(index)} label={formatX(date)} offset={9} tick={6} labelShift={band.step / 2} />
                ) : null,
              )}
              <XTick at={right} offset={9} tick={6} />
            </g>
            <g transform={"translate(" + area.left + ", 0)"}>
              <line className={styles.axisLine} y1={area.top} y2={bottom} strokeLinecap="square" />
              {yTickValues.map(value => (
                <YTick key={value} at={y(value)} label={formatY(value)} offset={8} tick={6} />
              ))}
            </g>
            <clipPath id={id + "-plot"}>
              <rect x={area.left} y={area.top} width={area.width} height={area.height} />
            </clipPath>
          </>
        )}
      </svg>
      <ChartDataTable
        caption={ariaLabel}
        columns={series.map(entry => entry.label)}
        rows={dates.map((date, index) => ({
          key: date,
          head: formatDate(date),
          cells: series.map(entry => cell(entry, index)),
        }))}
      />
      {hover && tooltip ? (
        <ChartTooltip at={hover.pointer} className={tooltipClassName}>
          {tooltip}
        </ChartTooltip>
      ) : null}
    </div>
  );
}
