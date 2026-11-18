import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Atlas, type AtlasViewport } from './Atlas';

function renderAtlas(place: { x?: number; y?: number }) {
  return openAtlas(place).center;
}

function openAtlas(props: { x?: number; y?: number; height?: number }) {
  const onChange = vi.fn<(viewport: AtlasViewport) => void>();
  render(
    <Atlas
      {...props}
      onChange={onChange}
    />,
  );
  return onChange.mock.calls[0][0];
}

/** Every element is `width` by `height` pixels. */
function layOut(width: number, height: number) {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    width,
    height,
    right: width,
    bottom: height,
    toJSON: () => ({}),
  } as DOMRect);
}

describe('Atlas', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe('when the caller gives no place', () => {
    it('should open with parcel 0,0 in the middle', () => {
      expect(renderAtlas({})).toEqual({ x: 0, y: 0 });
    });
  });

  describe('when the frame is narrower than it is high', () => {
    it('should size a parcel from the width, as the map does', () => {
      layOut(300, 480);

      // 14 pixels for each parcel, 340 parcels on the 300 pixels of the smaller side.
      expect(openAtlas({ height: 480 }).zoom).toBeCloseTo((14 * 340) / 300);
    });
  });

  describe('when the frame is wider than it is high', () => {
    it('should size a parcel from the height', () => {
      layOut(800, 480);

      expect(openAtlas({ height: 480 }).zoom).toBeCloseTo((14 * 340) / 480);
    });
  });

  describe('when the caller gives a place', () => {
    it('should open with that parcel in the middle', () => {
      expect(renderAtlas({ x: 12, y: -7 })).toEqual({ x: 12, y: -7 });
    });
  });
});
