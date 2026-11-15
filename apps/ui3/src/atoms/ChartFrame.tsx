import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import styles from "./chart.module.css";

// Shared parts of the SVG charts (LineChart, StackedBarChart): the measured width, the axis labels, the
// tooltip at the pointer and the data table for assistive technology.

export type ChartMargin = { left: number; right: number; top: number; bottom: number };
export type ChartArea = { left: number; top: number; width: number; height: number };
export type ChartPointer = { x: number; y: number };
export type ChartTooltipRow = { id: string; label: string; color: string; value: string };

export const DEFAULT_CHART_MARGIN: ChartMargin = { left: 48, right: 24, top: 12, bottom: 28 };
export const DEFAULT_CHART_HEIGHT = 300;

export const cx = (...names: Array<string | undefined | false>) => names.filter(Boolean).join(" ");

/** The chart takes the width of its container, in whole pixels, unless a fixed width is given. */
export function useChartWidth(fixed?: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [measured, setMeasured] = useState(0);

  useLayoutEffect(() => {
    const element = ref.current;
    if (fixed !== undefined || !element) return;
    const read = () => setMeasured(Math.floor(element.getBoundingClientRect().width));
    read();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(read);
    observer.observe(element);
    return () => observer.disconnect();
  }, [fixed]);

  return [ref, fixed ?? measured] as const;
}

export function chartArea(width: number, height: number, margin: ChartMargin): ChartArea {
  return {
    left: margin.left,
    top: margin.top,
    width: Math.max(0, width - margin.left - margin.right),
    height: Math.max(0, height - margin.top - margin.bottom),
  };
}

type GridProps = { area: ChartArea; ys: number[] };

export function ChartGrid({ area, ys }: GridProps) {
  return (
    <g>
      {ys.map(y => (
        <line key={y} className={styles.grid} x1={area.left} x2={area.left + area.width} y1={y} y2={y} />
      ))}
    </g>
  );
}

type TickProps = { at: number; label?: string; offset: number; tick?: number; labelShift?: number };

/** A label under the plot, `offset` pixels below the axis. `tick` draws a mark of that length. */
export function XTick({ at, label, offset, tick, labelShift = 0 }: TickProps) {
  return (
    <g transform={"translate(" + at + ", 0)"}>
      {tick ? <line className={styles.tick} y2={tick} /> : null}
      {label !== undefined && (
        <text className={styles.tickLabel} x={labelShift} y={offset} textAnchor="middle" dominantBaseline="hanging">
          {label}
        </text>
      )}
    </g>
  );
}

/**
 * A label left of the plot, `offset` pixels from the axis. `tick` draws a mark of that length.
 * The label sits in a `tspan`: without it the browser paints the glyphs on other sub-pixels.
 */
export function YTick({ at, label, offset, tick }: TickProps) {
  return (
    <g transform={"translate(0, " + at + ")"}>
      {tick ? <line className={styles.tick} x2={-tick} /> : null}
      <text className={styles.tickLabel} x={-offset} y={0} textAnchor="end" dominantBaseline="central">
        <tspan x={-offset} dy="0px" dominantBaseline="central">{label}</tspan>
      </text>
    </g>
  );
}

type TooltipProps = { at: ChartPointer; className?: string; children: ReactNode };

/**
 * A box whose top left corner is at the pointer (viewport coordinates), on whole device pixels. It goes to
 * the left of the pointer when the right side has no room, and moves up to stay in the viewport. It never
 * takes pointer events.
 */
export function ChartTooltip({ at, className, children }: TooltipProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const { width, height } = element.getBoundingClientRect();
    const viewWidth = document.documentElement.clientWidth;
    const viewHeight = document.documentElement.clientHeight;
    const ratio = window.devicePixelRatio || 1;
    const toDevicePixel = (value: number) => Math.round(value * ratio) / ratio;
    const left = toDevicePixel(at.x + width > viewWidth && at.x - width >= 0 ? at.x - width : at.x);
    const top = toDevicePixel(Math.max(0, Math.min(at.y, Math.max(viewHeight - height, at.y - height))));
    setPlace(current => (current && current.left === left && current.top === top ? current : { left, top }));
  });

  return createPortal(
    <div
      ref={ref}
      role="tooltip"
      className={cx(styles.tooltip, className)}
      style={{ left: place?.left ?? at.x, top: place?.top ?? at.y, visibility: place ? undefined : "hidden" }}
    >
      {children}
    </div>,
    document.body,
  );
}

/** The tooltip content when the caller gives none: a title, then a mark, a name and a value for each series. */
export function ChartTooltipTable({ title, rows }: { title: string; rows: ChartTooltipRow[] }) {
  return (
    <div className={styles.tooltipPaper}>
      <table className={styles.tooltipTable}>
        <thead>
          <tr>
            <td colSpan={3}>{title}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map(row => (
            <tr key={row.id}>
              <td>
                <span className={styles.tooltipMark} style={{ background: row.color }} />
              </td>
              <td className={styles.tooltipLabel}>{row.label}</td>
              <td className={styles.tooltipValue}>{row.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

type DataTableProps = {
  caption: string;
  columns: string[];
  rows: Array<{ key: string | number; head: string; cells: string[] }>;
};

/** The chart data as a table that only assistive technology reads. */
export function ChartDataTable({ caption, columns, rows }: DataTableProps) {
  return (
    <table className={styles.dataTable}>
      <caption>{caption}</caption>
      <thead>
        <tr>
          <td />
          {columns.map((column, index) => (
            <th key={index} scope="col">
              {column}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map(row => (
          <tr key={row.key}>
            <th scope="row">{row.head}</th>
            {row.cells.map((cell, index) => (
              <td key={index}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
