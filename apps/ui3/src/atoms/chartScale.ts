// Scale helpers for the SVG charts. The numbers follow the d3 algorithms (tick steps of 1, 2 and 5, "nice"
// domains, band padding), so a chart keeps the geometry of a d3-based one without the library.

const E10 = Math.sqrt(50);
const E5 = Math.sqrt(10);
const E2 = Math.sqrt(2);

function tickSpec(start: number, stop: number, count: number): [number, number, number] {
  const step = (stop - start) / Math.max(0, count);
  const power = Math.floor(Math.log10(step));
  const error = step / Math.pow(10, power);
  const factor = error >= E10 ? 10 : error >= E5 ? 5 : error >= E2 ? 2 : 1;
  let i1: number;
  let i2: number;
  let inc: number;
  if (power < 0) {
    inc = Math.pow(10, -power) / factor;
    i1 = Math.round(start * inc);
    i2 = Math.round(stop * inc);
    if (i1 / inc < start) ++i1;
    if (i2 / inc > stop) --i2;
    inc = -inc;
  } else {
    inc = Math.pow(10, power) * factor;
    i1 = Math.round(start / inc);
    i2 = Math.round(stop / inc);
    if (i1 * inc < start) ++i1;
    if (i2 * inc > stop) --i2;
  }
  if (i2 < i1 && 0.5 <= count && count < 2) return tickSpec(start, stop, count * 2);
  return [i1, i2, inc];
}

/** Round tick values between `start` and `stop`, about `count` of them. */
export function linearTicks(start: number, stop: number, count: number): number[] {
  if (!(count > 0) || !Number.isFinite(start) || !Number.isFinite(stop)) return [];
  if (start === stop) return [start];
  const [i1, i2, inc] = tickSpec(start, stop, count);
  if (!(i2 >= i1)) return [];
  const ticks: number[] = [];
  for (let i = i1; i <= i2; ++i) ticks.push(inc < 0 ? i / -inc : i * inc);
  return ticks;
}

/** Widens `[start, stop]` to round values, for about `count` ticks. */
export function niceLinear(start: number, stop: number, count: number): [number, number] {
  if (!(stop > start) || !(count > 0)) return [start, stop];
  let lo = start;
  let hi = stop;
  let previous: number | undefined;
  for (let pass = 0; pass < 10; ++pass) {
    const step = tickSpec(lo, hi, count)[2];
    if (step === previous) return [lo, hi];
    if (step > 0) {
      lo = Math.floor(lo / step) * step;
      hi = Math.ceil(hi / step) * step;
    } else if (step < 0) {
      lo = Math.ceil(lo * step) / step;
      hi = Math.floor(hi * step) / step;
    } else {
      break;
    }
    previous = step;
  }
  return [start, stop];
}

export type Scale = (value: number) => number;

/** Maps `[d0, d1]` on `[r0, r1]`. A domain with no extent maps on the middle of the range. */
export function linearScale(d0: number, d1: number, r0: number, r1: number): Scale {
  const span = d1 - d0;
  if (!span) return () => (r0 + r1) / 2;
  return value => {
    const t = (value - d0) / span;
    return r0 * (1 - t) + r1 * t;
  };
}

type Unit = { floor: (date: Date) => void; offset: (date: Date, step: number) => void; field?: (date: Date) => number };

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const midnight = (date: Date) => date.setHours(0, 0, 0, 0);

const second: Unit = {
  floor: date => date.setTime(date.getTime() - date.getMilliseconds()),
  offset: (date, step) => date.setTime(date.getTime() + step * SECOND),
  field: date => date.getUTCSeconds(),
};
const minute: Unit = {
  floor: date => date.setTime(date.getTime() - date.getMilliseconds() - date.getSeconds() * SECOND),
  offset: (date, step) => date.setTime(date.getTime() + step * MINUTE),
  field: date => date.getMinutes(),
};
const hour: Unit = {
  floor: date =>
    date.setTime(date.getTime() - date.getMilliseconds() - date.getSeconds() * SECOND - date.getMinutes() * MINUTE),
  offset: (date, step) => date.setTime(date.getTime() + step * HOUR),
  field: date => date.getHours(),
};
const day: Unit = {
  floor: midnight,
  offset: (date, step) => date.setDate(date.getDate() + step),
  field: date => date.getDate() - 1,
};
// Weeks start on Sunday.
const week: Unit = {
  floor: date => {
    date.setDate(date.getDate() - date.getDay());
    midnight(date);
  },
  offset: (date, step) => date.setDate(date.getDate() + step * 7),
};
const month: Unit = {
  floor: date => {
    date.setDate(1);
    midnight(date);
  },
  offset: (date, step) => date.setMonth(date.getMonth() + step),
  field: date => date.getMonth(),
};
const year: Unit = {
  floor: date => {
    date.setMonth(0, 1);
    midnight(date);
  },
  offset: (date, step) => date.setFullYear(date.getFullYear() + step),
};

const TIME_STEPS: Array<[Unit, number, number]> = [
  [second, 1, SECOND],
  [second, 5, 5 * SECOND],
  [second, 15, 15 * SECOND],
  [second, 30, 30 * SECOND],
  [minute, 1, MINUTE],
  [minute, 5, 5 * MINUTE],
  [minute, 15, 15 * MINUTE],
  [minute, 30, 30 * MINUTE],
  [hour, 1, HOUR],
  [hour, 3, 3 * HOUR],
  [hour, 6, 6 * HOUR],
  [hour, 12, 12 * HOUR],
  [day, 1, DAY],
  [day, 2, 2 * DAY],
  [week, 1, 7 * DAY],
  [month, 1, 30 * DAY],
  [month, 3, 90 * DAY],
  [year, 1, 365 * DAY],
];

/**
 * Widens a time domain (epoch milliseconds) to the calendar step that fits about `count` ticks: seconds to
 * years, in local time. A span under `count` seconds, or over `count` years, stays as it is.
 */
export function niceTime(start: number, stop: number, count: number): [number, number] {
  if (!(stop > start) || !(count > 0)) return [start, stop];
  const target = (stop - start) / count;
  const index = TIME_STEPS.findIndex(([, , duration]) => duration > target);
  const above = TIME_STEPS[index];
  const below = TIME_STEPS[index - 1];
  if (!above || !below) return [start, stop];
  const [unit, every] = target / below[2] < above[2] / target ? below : above;
  const passes = (date: Date) => every === 1 || !unit.field || unit.field(date) % every === 0;
  const floor = (time: number) => {
    const date = new Date(time);
    unit.floor(date);
    while (!passes(date)) {
      date.setTime(date.getTime() - 1);
      unit.floor(date);
    }
    return date;
  };
  const ceil = (time: number) => {
    const date = floor(time - 1);
    do unit.offset(date, 1);
    while (!passes(date));
    unit.floor(date);
    return date.getTime();
  };
  return [floor(start).getTime(), ceil(stop)];
}

export type BandScale = { start: (index: number) => number; step: number; bandwidth: number };

/** `count` bands on `[r0, r1]`. `gapRatio` is the part of each step left empty, half of it at each end. */
export function bandScale(count: number, r0: number, r1: number, gapRatio = 0): BandScale {
  const step = (r1 - r0) / Math.max(1, count);
  const first = r0 + (r1 - r0 - step * (count - gapRatio)) * 0.5;
  return { start: index => first + step * index, step, bandwidth: step * (1 - gapRatio) };
}

const round3 = (value: number) => Math.round(value * 1000) / 1000;

/** Straight segments through the points. A missing point (`null`) breaks the line; a lone point is a closed dot. */
export function linePath(points: Array<[number, number] | null>): string {
  let path = "";
  let run = 0;
  const close = () => {
    if (run === 1) path += "Z";
    run = 0;
  };
  for (const point of points) {
    if (!point) {
      close();
      continue;
    }
    path += (run === 0 ? "M" : "L") + round3(point[0]) + "," + round3(point[1]);
    ++run;
  }
  close();
  return path;
}

/** Index of the value nearest to `target` in a rising list; -1 for an empty list. */
export function nearestIndex(values: number[], target: number): number {
  let best = -1;
  let distance = Infinity;
  values.forEach((value, index) => {
    const d = Math.abs(value - target);
    if (d < distance || (d === distance && value > target)) {
      best = index;
      distance = d;
    }
  });
  return best;
}

/** Tick count for an axis of `length` pixels when none is asked for: one each 50 pixels, two at least. */
export const defaultTickCount = (length: number, asked?: number) =>
  Math.min(999, Math.max(2, asked ?? Math.floor(Math.abs(length) / 50)));
