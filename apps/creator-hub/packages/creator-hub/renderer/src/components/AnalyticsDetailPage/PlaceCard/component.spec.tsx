import { fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { PlaceDetails } from '/shared/types/place-analytics';
import { PlaceAccess } from '/shared/types/place-analytics';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../../tests/utils/TestProviders';
import { PlaceCard } from './component';

vi.mock('/@/hooks/useSnackbar', () => ({ useSnackbar: () => ({ pushGeneric: vi.fn() }) }));

const AVATAR = 'https://example.org/ada.png';
const OTHER_AVATAR = 'https://example.org/ada-2.png';

const place = (lastPublishedBy: PlaceDetails['lastPublishedBy']): PlaceDetails => ({
  placeId: 'world:bananarama.dcl.eth@0,0',
  name: 'Bananarama',
  thumbnail: 'https://example.org/thumbnail.png',
  likeRate: null,
  access: PlaceAccess.PUBLIC,
  publishedIn: 'bananarama.dcl.eth',
  location: { world: 'bananarama.dcl.eth', x: 0, y: 0 },
  lastPublishedBy,
  lastUpdatedAt: null,
});

const card = (avatar: string | null) => (
  <TestProviders>
    <PlaceCard place={place({ name: 'Zed', avatar })} />
  </TestProviders>
);

/** Both pictures of the card are decorative (`alt=""`), so the URL tells them apart. */
const avatarImage = (url: string) =>
  screen.queryAllByRole('presentation').find(image => image.getAttribute('src') === url);

describe('the publisher of a place', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  describe('when the publisher has an avatar', () => {
    it('should show the picture and not the first letter of the name', () => {
      render(card(AVATAR));

      expect(avatarImage(AVATAR)).toBeDefined();
      expect(screen.queryByText('Z')).toBeNull();
    });
  });

  describe('when the publisher has no avatar', () => {
    it('should show the first letter of the name', () => {
      render(card(null));

      expect(screen.getByText('Z')).toBeDefined();
    });
  });

  describe('when the avatar does not load', () => {
    it('should show the first letter of the name in place of the picture', () => {
      render(card(AVATAR));

      fireEvent.error(avatarImage(AVATAR)!);

      expect(avatarImage(AVATAR)).toBeUndefined();
      expect(screen.getByText('Z')).toBeDefined();
    });

    it('should try a new avatar address again', () => {
      const { rerender } = render(card(AVATAR));
      fireEvent.error(avatarImage(AVATAR)!);

      rerender(card(OTHER_AVATAR));

      expect(avatarImage(OTHER_AVATAR)).toBeDefined();
      expect(screen.queryByText('Z')).toBeNull();
    });
  });
});
