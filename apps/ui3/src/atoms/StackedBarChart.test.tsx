import { describe, expect, test } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";

import StackedBarChart, { type StackedBarChartProps } from "./StackedBarChart";

const dates = [0, 1, 2, 3, 4, 5, 6, 7].map(week => new Date(2026, 1, 2 + week * 7).getTime());
const short = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });
const long = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", year: "numeric" });

const base: StackedBarChartProps = {
  "aria-label": "Users by platform",
  dates,
  series: [
    { id: "desktop", label: "Desktop", color: "#6690f0", data: [120, 131, 125, 142, 150, 148, 163, 184] },
    { id: "mobile", label: "Mobile", color: "#17a06f", data: [80, 90, 85, 100, null, 110, 120, 129] },
    { id: "web", label: "Web", color: "#c9761f", data: [30, 31, 35, 42, 40, 0, 53, 55] },
  ],
  width: 696,
  height: 351,
  margin: { left: 51, right: 40, top: 9, bottom: 26 },
  gapRatio: 0.56,
  radius: 4,
  xTickFilter: (_date, index) => index % 2 === 0,
  formatX: date => short.format(date),
  formatDate: date => long.format(date),
};

const bars = (container: HTMLElement, id: string) => Array.from(container.querySelectorAll(`rect[data-series="${id}"]`));
const num = (node: Element | undefined, name: string) => Number(node?.getAttribute(name));

describe("StackedBarChart", () => {
  test("the chart is an image with a name, and its data is a table", () => {
    render(<StackedBarChart {...base} />);

    expect(screen.getByRole("img", { name: "Users by platform" })).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Users by platform" });
    expect(within(table).getByRole("row", { name: "Mar 2, 2026 150 - 40" })).toBeInTheDocument();
  });

  test("each date has one bar of the band width, and the segments stack from the zero line", () => {
    const { container } = render(<StackedBarChart {...base} />);

    const desktop = bars(container, "desktop");
    const mobile = bars(container, "mobile");
    const web = bars(container, "web");
    expect(desktop).toHaveLength(8);
    expect(num(desktop[0], "x")).toBe(72.175);
    expect(num(desktop[7], "x")).toBe(601.55);
    expect(num(desktop[0], "width")).toBe(33.275);
    // The axis ends at 400 (the highest stack is 368), so 1 unit is 0.79 pixels of the 316 pixel plot.
    expect(num(desktop[0], "y")).toBe(230.2);
    expect(num(desktop[0], "height")).toBe(94.8);
    expect(num(mobile[0], "y")).toBe(167);
    expect(num(mobile[0], "height")).toBe(63.2);
    expect(num(web[0], "y")).toBe(143.3);
    expect(num(web[0], "height")).toBe(23.7);
    expect(desktop[0]).toHaveAttribute("fill", "#6690f0");
  });

  test("a missing value takes no room in the stack", () => {
    const { container } = render(<StackedBarChart {...base} />);

    expect(num(bars(container, "mobile")[4], "height")).toBe(0);
    expect(num(bars(container, "web")[4], "y")).toBe(174.9);
  });

  test("the top of each bar is round and the segments are cut to that outline", () => {
    const { container } = render(<StackedBarChart {...base} />);

    const outline = container.querySelector("clipPath path") as SVGPathElement;
    expect(outline).toHaveAttribute("d", "M72.175,325V147.3A4,4 0 0 1 76.175,143.3H101.45A4,4 0 0 1 105.45,147.3V325Z");
    const clip = (bars(container, "web")[0]?.parentNode as Element).getAttribute("clip-path");
    expect(clip).toBe("url(#" + (outline.parentNode as Element).id + ")");
  });

  test("round ticks on the y axis and a label for each bar the filter lets through", () => {
    render(<StackedBarChart {...base} />);
    const svg = screen.getByRole("img");

    for (const label of ["0", "50", "200", "350", "400"]) expect(within(svg).getByText(label)).toBeInTheDocument();
    for (const label of ["Feb 2", "Feb 16", "Mar 2", "Mar 16"]) expect(within(svg).getByText(label)).toBeInTheDocument();
    expect(within(svg).queryByText("Feb 9")).toBeNull();
    expect(within(svg).getByText("Feb 16")).toHaveAttribute("x", "37.8125");
  });

  test("the slot of a bar opens one tooltip with every series", () => {
    const { container } = render(<StackedBarChart {...base} />);

    fireEvent.pointerMove(screen.getByRole("img"), { clientX: 360, clientY: 40 });

    const tip = screen.getByRole("tooltip");
    expect(within(tip).getByText("Mar 2, 2026")).toBeInTheDocument();
    expect(within(tip).getByRole("row", { name: "Desktop 150" })).toBeInTheDocument();
    expect(within(tip).getByRole("row", { name: "Mobile -" })).toBeInTheDocument();
    expect(within(tip).getByRole("row", { name: "Web 40" })).toBeInTheDocument();
    const band = container.querySelector("svg rect:not([data-series])") as SVGRectElement;
    expect(num(band, "x")).toBe(353.5);
    expect(num(band, "width")).toBe(75.625);

    fireEvent.pointerLeave(screen.getByRole("img"));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("the caller can render the tooltip", () => {
    render(<StackedBarChart {...base} renderTooltip={({ date, index }) => <b>{long.format(date) + " #" + index}</b>} />);

    fireEvent.pointerMove(screen.getByRole("img"), { clientX: 60, clientY: 300 });

    expect(screen.getByRole("tooltip")).toHaveTextContent("Feb 2, 2026 #0");
  });
});
