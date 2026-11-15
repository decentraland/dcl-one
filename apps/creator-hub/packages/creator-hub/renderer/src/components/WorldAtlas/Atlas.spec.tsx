import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Atlas, type AtlasViewport } from './Atlas';

function renderAtlas(place: { x?: number; y?: number }) {
  const onChange = vi.fn<(viewport: AtlasViewport) => void>();
  render(
    <Atlas
      {...place}
      onChange={onChange}
    />,
  );
  return onChange.mock.calls[0][0].center;
}

describe('Atlas', () => {
  afterEach(cleanup);

  describe('when the caller gives no place', () => {
    it('should open with parcel 0,0 in the middle', () => {
      expect(renderAtlas({})).toEqual({ x: 0, y: 0 });
    });
  });

  describe('when the caller gives a place', () => {
    it('should open with that parcel in the middle', () => {
      expect(renderAtlas({ x: 12, y: -7 })).toEqual({ x: 12, y: -7 });
    });
  });
});
