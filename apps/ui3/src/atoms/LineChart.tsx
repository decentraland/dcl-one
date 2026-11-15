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
  type ChartArea,
  type ChartMargin,
  type ChartPointer,
} from "./ChartFrame";
import {
  defaultTickCount,
  linePath,
  linearScale,
  linearTicks,
  nearestIndex,
  niceLinear,
  niceTime,
  type Scale,
} from "./chartScale";
import styles from "./chart.module.css";

export type LineChartSeries = {
  id: string;
  label: string;
  /** Flat colour of the series: the line, the hover mark and the tooltip swatch. */
  color: string;
  /** One value for each date; `null` breaks the line. */
  data: Array<number | null>;
  /** Paint of the line when it is not the flat colour, for example `url(#gradient)`. */
  paint?: string;
  /** SVG dash pattern of the line. */
  dash?: string;
  /** `hollow`: the hover mark takes the surface colour and the flat colour goes to its outline. */
  mark?: "solid" | "hollow";
};

export type LineChartHover = { index: number; date: number; series: LineChartSeries[] };

/** Positions in the SVG, for content the caller draws over the plot. */
export type LineChartScales = { x: Scale; y: Scale; area: ChartArea };

export type LineChartProps = {
  /** Names the chart. The same text is the caption of the data table. */
  "aria-label": string;
  /** Epoch milliseconds, rising, one for each point. */
  dates: number[];
  series: LineChartSeries[];
  height?: number;
  /** Fixed width in pixels. Without it the chart takes the width of its container. */
  width?: number;
  margin?: ChartMargin;
  /** Lower and upper bound of the y axis. A bound that is not given comes from the data, widened to a round value. */
  yMin?: number;
  yMax?: number;
  /** Explicit y ticks; otherwise about `yTickCount` round values. */
  yTicks?: number[];
  yTickCount?: number;
  /** Dates that take a label on the x axis. Default: every date. */
  xTicks?: number[];
  formatX?: (date: number) => string;
  formatY?: (value: number) => string;
  /** A dashed line across the plot with its label at the right end. */
  reference?: { value: number; label: string };
  /** Content of the hover tooltip. Default: a table with one row for each series. */
  renderTooltip?: (hover: LineChartHover) => ReactNode;
  /** Extra SVG content over the plot (end labels, gradient definitions). */
  children?: (scales: LineChartScales) => ReactNode;
  className?: string;
  tooltipClassName?: string;
};

const defaultFormatX = (date: number) => new Date(date).toLocaleDateString();
const defaultFormatY = (value: number) => String(value);

/**
 * Lines over a time axis: horizontal grid, labels on both axes, a mark and a tooltip for the date nearest to
 * the pointer. The x axis is a time scale, so uneven dates keep their distance.
 */
export default function LineChart({
  "aria-label": ariaLabel,
  dates,
  series,
  height = DEFAULT_CHART_HEIGHT,
  width: fixedWidth,
  margin = DEFAULT_CHART_MARGIN,
  yMin,
  yMax,
  yTicks,
  yTickCount,
  xTicks,
  formatX = defaultFormatX,
  formatY = defaultFormatY,
  reference,
  renderTooltip,
  children,
  className,
  tooltipClassName,
}: LineChartProps) {
  const clipId = "chart-clip-" + useId().replace(/:/g, "");
  const [ref, width] = useChartWidth(fixedWidth);
  const [hovered, setHover] = useState<{ index: number; pointer: ChartPointer } | null>(null);
  const hoverDate = hovered ? dates[hovered.index] : undefined;
  const hover = hovered && hoverDate !== undefined ? { ...hovered, date: hoverDate } : null;

  const area = chartArea(width, height, margin);
  const ready = area.width > 0 && area.height > 0 && dates.length > 0;
  const right = area.left + area.width;
  const bottom = area.top + area.height;

  const values = series.flatMap(entry => entry.data.filter((value): value is number => value !== null));
  const dataMin = values.length ? Math.min(...values) : 0;
  const dataMax = values.length ? Math.max(...values) : 1;
  const yCount = defaultTickCount(area.height, yTickCount);
  const niceY = niceLinear(yMin ?? dataMin, yMax ?? dataMax, yCount);
  const yDomain: [number, number] = [yMin ?? niceY[0], yMax ?? niceY[1]];
  const y = linearScale(yDomain[0], yDomain[1], bottom, area.top);
  const yTickValues = yTicks ?? linearTicks(yDomain[0], yDomain[1], yCount);

  const [x0, x1] = dates.length
    ? niceTime(Math.min(...dates), Math.max(...dates), defaultTickCount(area.width))
    : [0, 1];
  const x = linearScale(x0, x1, area.left, right);
  const invertX = linearScale(area.left, right, x0, x1);

  const point = (date: number | undefined, value: number | null): [number, number] | null =>
    date === undefined || value === null ? null : [x(date), y(value)];

  const onPointerMove = (event: PointerEvent<SVGSVGElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    const px = event.clientX - box.left;
    const py = event.clientY - box.top;
    if (!ready || px < area.left || px > right || py < area.top || py > bottom) {
      setHover(null);
      return;
    }
    const index = nearestIndex(dates, invertX(px));
    setHover(index < 0 ? null : { index, pointer: { x: event.clientX, y: event.clientY } });
  };

  const tooltip =
    hover &&
    (renderTooltip ? (
      renderTooltip({ index: hover.index, date: hover.date, series })
    ) : (
      <ChartTooltipTable
        title={formatX(hover.date)}
        rows={series.map(entry => {
          const value = entry.data[hover.index] ?? null;
          return { id: entry.id, label: entry.label, color: entry.color, value: value === null ? "-" : formatY(value) };
        })}
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
            <g clipPath={"url(#" + clipId + ")"}>
              {series.map(entry => (
                <path
                  key={entry.id}
                  className={styles.line}
                  data-series={entry.id}
                  d={linePath(entry.data.map((value, index) => point(dates[index], value)))}
                  stroke={entry.paint ?? entry.color}
                  strokeDasharray={entry.dash}
                />
              ))}
            </g>
            <g transform={"translate(0, " + bottom + ")"}>
              {(xTicks ?? dates).map(date => (
                <XTick key={date} at={x(date)} label={formatX(date)} offset={7} />
              ))}
            </g>
            <g transform={"translate(" + area.left + ", 0)"}>
              {yTickValues.map(value => (
                <YTick key={value} at={y(value)} label={formatY(value)} offset={6} />
              ))}
            </g>
            {hover &&
              series.map(entry => {
                const value = entry.data[hover.index] ?? null;
                if (value === null) return null;
                const hollow = entry.mark === "hollow";
                return (
                  <circle
                    key={entry.id}
                    className={styles.mark}
                    data-mark={hollow ? "hollow" : "solid"}
                    cx={x(hover.date)}
                    cy={y(value)}
                    r={6}
                    fill={hollow ? undefined : entry.color}
                    style={hollow ? { stroke: entry.color } : undefined}
                  />
                );
              })}
            <clipPath id={clipId}>
              <rect x={area.left} y={area.top} width={area.width} height={area.height} />
            </clipPath>
            {reference && (
              <g className={styles.reference} data-chart-reference="">
                <path className={styles.referenceLine} d={"M " + area.left + " " + y(reference.value) + " l " + area.width + " 0"} />
                <text className={styles.referenceLabel} x={right - 5} y={y(reference.value) - 5} textAnchor="end">
                  {reference.label}
                </text>
              </g>
            )}
            {children?.({ x, y, area })}
          </>
        )}
      </svg>
      <ChartDataTable
        caption={ariaLabel}
        columns={series.map(entry => entry.label)}
        rows={dates.map((date, index) => ({
          key: date,
          head: formatX(date),
          cells: series.map(entry => {
            const value = entry.data[index] ?? null;
            return value === null ? "-" : formatY(value);
          }),
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
