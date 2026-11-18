import { fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { PlaceAnalyticsSummary } from '/shared/types/place-analytics';

import { flatten } from '/@/modules/store/translation/flatten';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../../tests/utils/TestProviders';
import { PlacesTable } from './component';

const plaza: PlaceAnalyticsSummary = {
  placeId: 'world:plaza.dcl.eth',
  name: 'Plaza',
  thumbnail: 'https://example.org/thumbnail.png',
  totalVisits: 12,
  day7Retention: 40,
  medianPlaytime: 5,
  concurrentUsers: 1,
  hasNoData: false,
};

function renderTable() {
  const onSelectPlace = vi.fn();
  const onTogglePin = vi.fn();
  render(
    <TestProviders>
      <PlacesTable
        places={[plaza]}
        pinnedPlaceIds={[]}
        onTogglePin={onTogglePin}
        onSelectPlace={onSelectPlace}
      />
    </TestProviders>,
  );
  return { onSelectPlace, onTogglePin };
}

const row = () => screen.getByRole('button', { name: 'Plaza' });
const options = () => screen.getByRole('button', { name: 'Options for Plaza' });

describe('PlacesTable', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  describe('when a key is pressed on the row', () => {
    it.each(['Enter', ' '])('should open the place on %j', key => {
      const { onSelectPlace } = renderTable();

      fireEvent.keyDown(row(), { key });

      expect(onSelectPlace).toHaveBeenCalledWith(plaza.placeId);
    });
  });

  describe('when a key is pressed on the options button of the row', () => {
    it.each(['Enter', ' '])('should not open the place on %j', key => {
      const { onSelectPlace } = renderTable();

      fireEvent.keyDown(options(), { key });

      expect(onSelectPlace).not.toHaveBeenCalled();
    });
  });

  describe('when a key is pressed on an item of the options menu', () => {
    it.each(['Enter', ' '])('should not open the place on %j', key => {
      const { onSelectPlace } = renderTable();
      fireEvent.click(options());

      fireEvent.keyDown(screen.getByRole('menuitem', { name: 'Pin to Watchlist' }), { key });

      expect(onSelectPlace).not.toHaveBeenCalled();
    });
  });
});
