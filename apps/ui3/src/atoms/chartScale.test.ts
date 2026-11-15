import { describe, expect, test } from "vitest";

import { bandScale, defaultTickCount, linePath, linearScale, linearTicks, nearestIndex, niceLinear, niceTime } from "./chartScale";

const day = (month: number, date: number) => new Date(2026, month, date).getTime();

describe("chartScale", () => {
  test("ticks are round values in steps of 1, 2 or 5", () => {
    expect(linearTicks(0, 250, 5)).toEqual([0, 50, 100, 150, 200, 250]);
    expect(linearTicks(0, 400, 6)).toEqual([0, 50, 100, 150, 200, 250, 300, 350, 400]);
    expect(linearTicks(0, 2.5, 5)).toEqual([0, 0.5, 1, 1.5, 2, 2.5]);
    expect(linearTicks(0, 26, 5)).toEqual([0, 5, 10, 15, 20, 25]);
    expect(linearTicks(0, 100, 5)).toEqual([0, 20, 40, 60, 80, 100]);
  });

  test("ticks of an empty or broken range", () => {
    expect(linearTicks(3, 3, 5)).toEqual([3]);
    expect(linearTicks(0, 10, 0)).toEqual([]);
    expect(linearTicks(0, Infinity, 5)).toEqual([]);
  });

  test("a nice domain ends on a tick", () => {
    expect(niceLinear(0, 248, 5)).toEqual([0, 250]);
    expect(niceLinear(0, 1840, 5)).toEqual([0, 2000]);
    expect(niceLinear(0, 2.3, 5)).toEqual([0, 2.5]);
    expect(niceLinear(0, 368, 6)).toEqual([0, 400]);
    expect(niceLinear(0, 0, 5)).toEqual([0, 0]);
  });

  test("a linear scale maps the domain on the range, also when the range goes down", () => {
    const y = linearScale(0, 250, 325, 9);
    expect(y(0)).toBe(325);
    expect(y(250)).toBe(9);
    expect(y(142)).toBeCloseTo(145.512, 3);
    expect(linearScale(5, 5, 0, 100)(5)).toBe(50);
  });

  test("eight weeks widen to whole weeks that start on Sunday", () => {
    expect(niceTime(day(1, 2), day(2, 23), 12)).toEqual([day(1, 1), day(2, 29)]);
  });

  test("three weeks widen to steps of two days in the month", () => {
    expect(niceTime(day(1, 2), day(1, 23), 12)).toEqual([day(1, 1), day(1, 23)]);
  });

  test("two years widen to whole quarters, and a domain with no extent stays", () => {
    expect(niceTime(day(1, 2), new Date(2028, 1, 2).getTime(), 12)).toEqual([day(0, 1), new Date(2028, 3, 1).getTime()]);
    expect(niceTime(day(1, 2), day(1, 2), 12)).toEqual([day(1, 2), day(1, 2)]);
  });

  test("bands leave the gap ratio empty, half of it at each end", () => {
    const band = bandScale(8, 51, 656, 0.56);
    expect(band.step).toBeCloseTo(75.625, 6);
    expect(band.bandwidth).toBeCloseTo(33.275, 6);
    expect(band.start(0)).toBeCloseTo(72.175, 6);
    expect(band.start(7)).toBeCloseTo(601.55, 6);
  });

  test("a missing point breaks the line and a lone point is a closed dot", () => {
    expect(linePath([[1, 2], [3.00049, 4.0006], null, [5, 6]])).toBe("M1,2L3,4.001M5,6Z");
    expect(linePath([null, null])).toBe("");
  });

  test("the nearest index, the later one on a tie", () => {
    expect(nearestIndex([10, 20, 30], 14)).toBe(0);
    expect(nearestIndex([10, 20, 30], 15)).toBe(1);
    expect(nearestIndex([10, 20, 30], 99)).toBe(2);
    expect(nearestIndex([], 1)).toBe(-1);
  });

  test("the default tick count is one each 50 pixels, two at least", () => {
    expect(defaultTickCount(316)).toBe(6);
    expect(defaultTickCount(60)).toBe(2);
    expect(defaultTickCount(316, 5)).toBe(5);
  });
});
