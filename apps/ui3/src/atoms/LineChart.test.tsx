import { afterEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";

import { ChartTooltip } from "./ChartFrame";
import LineChart, { type LineChartProps } from "./LineChart";

const dates = [0, 1, 2, 3, 4, 5, 6, 7].map(week => new Date(2026, 1, 2 + week * 7).getTime());
const short = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });
const users = { id: "users", label: "Weekly users", color: "#ff2d55", data: [142, 168, 155, 190, 214, 203, 231, 248] };

const base: LineChartProps = {
  "aria-label": "Weekly users",
  dates,
  series: [users],
  width: 696,
  height: 351,
  margin: { left: 51, right: 40, top: 9, bottom: 26 },
  yMin: 0,
  yTickCount: 5,
  xTicks: dates.filter((_date, index) => index % 2 === 0),
  formatX: date => short.format(date),
};

const line = (container: HTMLElement, id: string) => container.querySelector(`path[data-series="${id}"]`);

function hoverAt(x: number, y: number) {
  fireEvent.pointerMove(screen.getByRole("img"), { clientX: x, clientY: y });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("LineChart", () => {
  test("the chart is an image with a name, and its data is a table", () => {
    render(<LineChart {...base} />);

    expect(screen.getByRole("img", { name: "Weekly users" })).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Weekly users" });
    expect(within(table).getByRole("columnheader", { name: "Weekly users" })).toBeInTheDocument();
    expect(within(table).getByRole("row", { name: "Mar 23 248" })).toBeInTheDocument();
  });

  test("the line goes through each point on a time axis widened to whole weeks", () => {
    const { container } = render(<LineChart {...base} />);

    expect(line(container, "users")).toHaveAttribute(
      "d",
      "M61.804,145.512L137.429,112.648L213.054,129.08L288.679,84.84L364.304,54.504L439.929,68.408L515.554,33.016L591.179,11.528",
    );
    expect(line(container, "users")).toHaveAttribute("stroke", "#ff2d55");
  });

  test("the y axis starts at yMin and ends on a round value, with a grid line for each tick", () => {
    const { container } = render(<LineChart {...base} />);
    const svg = screen.getByRole("img");

    for (const label of ["0", "50", "100", "150", "200", "250"]) expect(within(svg).getByText(label)).toBeInTheDocument();
    for (const label of ["Feb 2", "Feb 16", "Mar 2", "Mar 16"]) expect(within(svg).getByText(label)).toBeInTheDocument();
    expect(within(svg).queryByText("Feb 9")).toBeNull();
    const grid = Array.from(container.querySelectorAll("svg > g:first-of-type line")).map(node => node.getAttribute("y1"));
    expect(grid.map(Number).map(value => Math.round(value * 10) / 10)).toEqual([325, 261.8, 198.6, 135.4, 72.2, 9]);
  });

  test("a given yMax is the top of the axis and explicit ticks replace the round ones", () => {
    const { container } = render(
      <LineChart {...base} yMax={100} yTicks={[0, 25, 50, 75, 100]} series={[{ ...users, data: [10, 20, 30, 40, 50, 60, 70, 80] }]} />,
    );
    const svg = screen.getByRole("img");

    expect(line(container, "users")?.getAttribute("d")).toMatch(/^M61\.804,293\.4L.*L591\.179,72\.2$/);
    expect(within(svg).getByText("75")).toBeInTheDocument();
    expect(within(svg).queryByText("80")).toBeNull();
  });

  test("a missing value breaks the line, and a series can take a paint and a dash pattern", () => {
    const { container } = render(
      <LineChart
        {...base}
        yMin={0}
        yMax={26}
        series={[
          { id: "settling", label: "Day 7", color: "#ff2d55", paint: "url(#g)", dash: "0 6", mark: "hollow", data: [null, null, null, null, null, 22, 3, 0] },
          { id: "final", label: "Day 7", color: "#ff2d55", paint: "url(#g)", data: [21, 19, 23, 26, 24, 22, null, null] },
        ]}
      />,
    );

    expect(line(container, "settling")).toHaveAttribute("d", "M439.929,57.615L515.554,288.538L591.179,325");
    expect(line(container, "settling")).toHaveAttribute("stroke-dasharray", "0 6");
    expect(line(container, "final")).toHaveAttribute("stroke", "url(#g)");
    expect(line(container, "final")).toHaveAttribute(
      "d",
      "M61.804,69.769L137.429,94.077L213.054,45.462L288.679,9L364.304,33.308L439.929,57.615",
    );
  });

  test("the reference line crosses the plot with its label at the right end", () => {
    const { container } = render(<LineChart {...base} yMax={26} reference={{ value: 20, label: "20% target" }} />);

    const reference = container.querySelector("[data-chart-reference]") as SVGGElement;
    expect(reference.querySelector("path")?.getAttribute("d")).toMatch(/^M 51 81\.923\d* l 605 0$/);
    const label = within(reference as unknown as HTMLElement).getByText("20% target");
    expect(label).toHaveAttribute("x", "651");
    expect(Number(label.getAttribute("y"))).toBeCloseTo(76.923, 3);
  });

  test("the children take the scales of the plot", () => {
    render(
      <LineChart {...base}>
        {({ x, y, area }) => (
          <text data-testid="probe" x={Math.round(x(dates[7] as number))} y={Math.round(y(248))}>
            {area.left + "," + area.top + "," + area.width + "," + area.height}
          </text>
        )}
      </LineChart>,
    );

    const probe = screen.getByTestId("probe");
    expect(probe).toHaveTextContent("51,9,605,316");
    expect(probe).toHaveAttribute("x", "591");
    expect(probe).toHaveAttribute("y", "12");
  });

  test("the pointer opens a tooltip for the nearest date and marks its point", () => {
    const { container } = render(<LineChart {...base} />);

    hoverAt(348, 175);

    const tip = screen.getByRole("tooltip");
    expect(within(tip).getByText("Mar 2")).toBeInTheDocument();
    expect(within(tip).getByText("214")).toBeInTheDocument();
    expect(tip.style.left).toBe("348px");
    expect(tip.style.top).toBe("175px");
    const mark = container.querySelector("circle") as SVGCircleElement;
    expect(Number(mark.getAttribute("cx"))).toBeCloseTo(364.304, 3);
    expect(mark).toHaveAttribute("fill", "#ff2d55");
  });

  test("the tooltip sits on whole device pixels", () => {
    vi.stubGlobal("devicePixelRatio", 2);
    render(<ChartTooltip at={{ x: 348.3, y: 175.63 }}>Mar 2</ChartTooltip>);

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe("348.5px");
    expect(tip.style.top).toBe("175.5px");
  });

  test("the tooltip closes when the pointer leaves the plot area or the chart", () => {
    render(<LineChart {...base} />);

    hoverAt(348, 175);
    hoverAt(20, 175);
    expect(screen.queryByRole("tooltip")).toBeNull();

    hoverAt(348, 175);
    fireEvent.pointerLeave(screen.getByRole("img"));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("the caller can render the tooltip, and a hollow mark takes the flat colour on its outline", () => {
    const { container } = render(
      <LineChart
        {...base}
        series={[{ ...users, mark: "hollow", paint: "url(#g)" }]}
        tooltipClassName="host-tip"
        renderTooltip={({ index, date, series }) => <b>{short.format(date) + ": " + series[0]?.data[index]}</b>}
      />,
    );

    hoverAt(600, 100);

    const tip = screen.getByRole("tooltip");
    expect(tip).toHaveTextContent("Mar 23: 248");
    expect(tip).toHaveClass("host-tip");
    const mark = container.querySelector("circle") as SVGCircleElement;
    expect(mark).toHaveAttribute("data-mark", "hollow");
    expect(mark).not.toHaveAttribute("fill");
    expect(mark.style.stroke).toBe("rgb(255, 45, 85)");
  });

  test("no plot without dates, and the image keeps its name", () => {
    const { container } = render(<LineChart {...base} dates={[]} series={[{ ...users, data: [] }]} />);

    expect(screen.getByRole("img", { name: "Weekly users" })).toBeInTheDocument();
    expect(container.querySelector("path")).toBeNull();
  });
});
