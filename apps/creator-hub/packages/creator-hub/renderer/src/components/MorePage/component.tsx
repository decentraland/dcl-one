import React from 'react';
import { cx } from '/@/lib/cx';
import Button from '@ui/atoms/HubButton';
import Text from '@ui/atoms/Text';

import { misc } from '#preload';

import { t } from '/@/modules/store/translation/utils';
import { CREATE_EVENT_URL } from '/@/modules/utils';

import CollectionsPng from '/assets/images/collections.png';
import NamesPng from '/assets/images/names.png';
import LandPng from '/assets/images/land.png';
import NewEventPng from '/assets/images/new-event.png';

import { Navbar, NavbarItem } from '../Navbar';

import './styles.css';

const BUILDER_URL = 'https://decentraland.org/builder';
const COLLECTIONS_URL = 'https://decentraland.org/create/collections';
const NAMES_URL = 'https://decentraland.org/shop/items?category=names';

const HorizontalCardWithImage: React.FC<{
  className?: string;
  title: string;
  description: string;
  image: string;
  action: () => void;
}> = React.memo(({ className, title, description, image, action }) => (
  <div className={cx('HorizontalCardWithImage', className)}>
    <div className="CardImage">
      <img src={image} />
    </div>
    <div className="CardContent">
      <div>
        <Text
          variant="h6"
          className="title"
        >
          {title}
        </Text>
        <Text variant="body2">{description}</Text>
      </div>
      <div className="CardActions">
        <Button
          className="CardAction"
          variant="text"
          onClick={action}
        >
          {t('more.cards.open_in_browser')}
        </Button>
      </div>
    </div>
  </div>
));

export function MorePage() {
  return (
    <main className="MorePage">
      <Navbar active={NavbarItem.MORE} />
      <div className="MoreContainer">
        <Text
          variant="h3"
          className="PageTitle"
        >
          {t('more.header.title')}
        </Text>
        <Text
          className="CardsSection"
          variant="h6"
        >
          {t('more.cards.create.title')}
        </Text>
        <div className="CardsGrid CardsGridSpaced">
          <div className="CardsGridItem">
            <HorizontalCardWithImage
              title={t('more.cards.create.submit_event.title')}
              description={t('more.cards.create.submit_event.description')}
              action={() => misc.openExternal(CREATE_EVENT_URL)}
              image={NewEventPng}
            />
          </div>
          <div className="CardsGridItem">
            <HorizontalCardWithImage
              title={t('more.cards.create.collections.title')}
              description={t('more.cards.create.collections.description')}
              action={() => misc.openExternal(COLLECTIONS_URL)}
              image={CollectionsPng}
            />
          </div>
        </div>
        <Text
          className="CardsSection"
          variant="h6"
        >
          {t('more.cards.manage.title')}
        </Text>
        <div className="CardsGrid">
          <div className="CardsGridItem">
            <HorizontalCardWithImage
              title={t('more.cards.manage.names.title')}
              description={t('more.cards.manage.names.description')}
              action={() => misc.openExternal(NAMES_URL)}
              image={NamesPng}
            />
          </div>
          <div className="CardsGridItem">
            <HorizontalCardWithImage
              title={t('more.cards.manage.land.title')}
              description={t('more.cards.manage.land.description')}
              action={() => misc.openExternal(`${BUILDER_URL}/land`)}
              image={LandPng}
            />
          </div>
        </div>
      </div>
    </main>
  );
}
