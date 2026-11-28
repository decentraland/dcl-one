import { useRef, useState } from 'react'
import {
  BackpackUpperBody,
  BlogBanner,
  BlogCard,
  Button,
  CardActionButton,
  CommunityCard,
  EquippedItemCard,
  EventCard,
  EventCardAction,
  EventCardActionLink,
  EventCardHost,
  GeneralReminderOn,
  JumpInCTA,
  MenusShare,
  PlaceCard,
  ShopCard,
  ShopCardPrice,
  ShopCardSkeleton,
  WearableCard,
  type ShopCardRarity,
  type WearableCardRarity
} from '@dcl/one-ui'
import type { GalleryEntry } from '../registry'
import { covers, people } from './data'
import layout from './layout.module.css'

const host = { name: people.ada.name, avatar: people.ada.picture, profileHref: '#ada' }
const rarities: ShopCardRarity[] = [
  'base',
  'common',
  'rare',
  'epic',
  'legendary',
  'mythic',
  'unique'
]

const itemRarities: WearableCardRarity[] = [
  'common',
  'uncommon',
  'rare',
  'epic',
  'legendary',
  'mythic',
  'unique',
  'exotic',
  'base'
]
const glyph = <BackpackUpperBody size={16} aria-hidden="true" />
const titled = (rarity: string) => `${rarity.charAt(0).toUpperCase()}${rarity.slice(1)}`

function SelectableWearableCards() {
  const [selected, setSelected] = useState('rare')
  return (
    <div className={layout.tiles}>
      {(['epic', 'rare', 'common'] as const).map((rarity, index) => (
        <WearableCard
          key={rarity}
          pick
          name={`${titled(rarity)} gloves`}
          thumbnail={covers.wearable}
          rarity={rarity}
          categoryIcon={glyph}
          count={index === 0 ? 3 : undefined}
          coverNote={index === 2 ? 'Equipped' : undefined}
          selected={selected === rarity}
          onClick={() => setSelected(rarity)}
        />
      ))}
    </div>
  )
}

const dayActions = (
  <>
    <EventCardAction
      aria-label="Remind me"
      label="Remind me"
      icon={<GeneralReminderOn size={20} aria-hidden="true" />}
    />
    <EventCardActionLink
      aria-label="Share"
      href="#share"
      icon={<MenusShare size={20} aria-hidden="true" />}
    />
  </>
)

function SelectableShopCard() {
  const [selected, setSelected] = useState(false)
  return (
    <ShopCard
      name="Selectable gloves"
      image={covers.wearable}
      rarity="rare"
      selected={selected}
      onSelect={() => setSelected((value) => !value)}
      price={<ShopCardPrice kind="credits" amount="120" />}
    />
  )
}

const blogPosts = [
  {
    title: "What's new in Decentraland this season",
    cover: covers.blog,
    category: 'Announcements',
    date: 'Jan 2, 2026'
  },
  {
    title: 'A walk through Genesis Plaza',
    cover: covers.plaza,
    category: 'Guides',
    date: 'Dec 18, 2025'
  },
  {
    title: 'Career Mondays: meet the builders',
    cover: covers.festival,
    category: 'Events',
    date: 'Dec 4, 2025'
  }
]

// A title that takes two lines in every grid column, from a phone to the widest desktop: what a loading card reserves.
const loadedTitle = 'Creator Fund: rewards for indie game creators'

// Keyboard focus draws the card's hover treatment. The page never moves focus on its own: the reviewer asks for it, as
// keyboard users reach the card by Tab.
function FocusedBlogCard() {
  const card = useRef<HTMLUListElement>(null)
  return (
    <div className={layout.blogGrids}>
      <div className={layout.row}>
        <Button
          variant="secondary"
          size="sm"
          label="Focus the card"
          onClick={() =>
            card.current
              ?.querySelector<HTMLAnchorElement>('h3 a')
              ?.focus({ preventScroll: true, focusVisible: true } as FocusOptions)
          }
        />
      </div>
      <ul ref={card} className={layout.blogIndex}>
        <li>
          <BlogCard
            title="Focused like a hovered card"
            href="#post"
            cover={<img src={covers.gallery} alt="" />}
            category="Events"
            metadata="Nov 20, 2025"
            author={people.grace.name}
          />
        </li>
      </ul>
    </div>
  )
}

export const cardEntries: GalleryEntry[] = [
  {
    name: 'EventCard',
    group: 'Cards',
    summary:
      'An event in a list (small) or a day column (day), with its host and actions. The cover uses the internal CardImage.',
    components: ['EventCard', 'EventCardHost', 'EventCardAction', 'EventCardActionLink'],
    examples: [
      {
        id: 'small',
        title: 'Small, upcoming',
        render: () => (
          <div className={layout.cards}>
            <EventCard
              title="Morning yoga"
              href="#event"
              image={covers.gallery}
              creator={host}
              time={{ label: 'Tomorrow, 9:00', dateTime: '2026-01-02T09:00:00Z' }}
              actions={
                <EventCardAction
                  aria-label="Remind me"
                  icon={<GeneralReminderOn size={24} aria-hidden="true" />}
                />
              }
            />
          </div>
        )
      },
      {
        id: 'small-live',
        title: 'Small, live with players and JUMP IN',
        render: () => (
          <div className={layout.cards}>
            <EventCard
              title="Music festival main stage"
              href="#event"
              image={covers.festival}
              creator={host}
              live
              players={128}
              playersLabel="128 online"
              actions={<JumpInCTA size="sm" href="#jump" />}
            />
          </div>
        )
      },
      {
        id: 'day',
        title: 'Day, with cover, host and actions',
        render: () => (
          <div className={layout.cards}>
            <EventCard
              variant="day"
              title="Art gallery opening"
              href="#event"
              image={covers.gallery}
              creator={host}
              time={{ label: '18:00' }}
              actions={dayActions}
            />
            <EventCard
              variant="day"
              title="Without a cover"
              href="#event"
              creator={{ name: people.grace.name }}
              time={{ label: '20:00' }}
            />
          </div>
        )
      },
      {
        id: 'announcement',
        title: 'Visible announcement',
        render: () => (
          <div className={layout.cards}>
            <EventCard
              variant="day"
              title="Reminder set"
              href="#event"
              image={covers.plaza}
              time={{ label: '21:00' }}
              announcement="We will remind you before it starts."
              showAnnouncement
            />
          </div>
        )
      },
      {
        id: 'host',
        title: 'EventCardHost on its own',
        render: () => (
          <div className={layout.stack}>
            <EventCardHost creator={host} />
            <EventCardHost creator={{ name: people.grace.name }} byLabel="hosted by" />
          </div>
        )
      }
    ]
  },
  {
    name: 'CommunityCard',
    group: 'Cards',
    summary:
      'A community in a grid. Hover or keyboard focus shrinks the cover and reveals the description.',
    components: ['CommunityCard'],
    examples: [
      {
        id: 'privacy',
        title: 'Public and private',
        render: () => (
          <div className={layout.cards}>
            <CommunityCard
              name="Builders Guild"
              href="#community"
              image={covers.plaza}
              ownerName={people.ada.name}
              membersLabel="1,340 members"
              description="Scene creators sharing tips, assets and feedback."
            />
            <CommunityCard
              name="Night Owls"
              href="#community"
              image={covers.festival}
              ownerName={people.grace.name}
              privacy="private"
              membersLabel="58 members"
              description="Late-night events, invite only."
            />
          </div>
        )
      },
      {
        id: 'minimal',
        title: 'No cover, owner, count or link, and custom labels',
        render: () => (
          <div className={layout.cards}>
            <CommunityCard name="New community" />
            <CommunityCard
              name="Comunidad"
              href="#community"
              image={covers.gallery}
              privacy="private"
              labels={{ public: 'Pública', private: 'Privada', view: 'Ver' }}
            />
          </div>
        )
      },
      {
        id: 'loading',
        title: 'Loading',
        render: () => (
          <div className={layout.cards}>
            <CommunityCard loading loadingLabel="Loading community" />
          </div>
        )
      }
    ]
  },
  {
    name: 'PlaceCard',
    group: 'Cards',
    summary: 'A place in the Explore grid or the Live Now rail.',
    components: ['PlaceCard'],
    examples: [
      {
        id: 'grid',
        title: 'Grid: default, featured and live',
        render: () => (
          <div className={layout.cards}>
            <PlaceCard
              title="Genesis Plaza"
              href="#place"
              jumpHref="#jump"
              image={covers.plaza}
              coords="0,0"
              creator={{ name: people.ada.name, avatar: people.ada.picture }}
              players={12}
              playersLabel="12 online"
            />
            <PlaceCard
              title="Featured gallery"
              href="#place"
              jumpHref="#jump"
              image={covers.gallery}
              coords="-30,12"
              featured
            />
            <PlaceCard
              title="Festival grounds"
              href="#place"
              jumpHref="#jump"
              image={covers.festival}
              coords="52,-8"
              live
              liveEventName="Music festival"
              players={240}
            />
          </div>
        )
      },
      {
        id: 'live',
        title: 'Live rail variant',
        render: () => (
          <div className={layout.cards}>
            <PlaceCard
              variant="live"
              title="Festival grounds"
              href="#place"
              jumpHref="#jump"
              image={covers.festival}
              live
              players={240}
              creator={{ name: people.grace.name, avatar: people.grace.picture }}
            />
          </div>
        )
      },
      {
        id: 'states',
        title: 'Loading, no JUMP IN and no cover',
        render: () => (
          <div className={layout.cards}>
            <PlaceCard loading loadingLabel="Loading place" />
            <PlaceCard title="Details only" href="#place" image={covers.plaza} coords="10,10" />
            <PlaceCard title="No cover" coords="1,1" />
          </div>
        )
      }
    ]
  },
  {
    name: 'ShopCard',
    group: 'Cards',
    summary: 'An item in the Shop or a catalogue grid, with its price and loading skeleton.',
    components: ['ShopCard', 'ShopCardPrice', 'ShopCardSkeleton'],
    examples: [
      {
        id: 'rarities',
        title: 'Rarities',
        render: () => (
          <div className={layout.cards}>
            {rarities.map((rarity) => (
              <ShopCard
                key={rarity}
                name={`${rarity} gloves`}
                image={covers.wearable}
                rarity={rarity}
                categoryIcon={<BackpackUpperBody size={16} aria-hidden="true" />}
                creator={people.ada}
                price={<ShopCardPrice kind="credits" amount="120" />}
                href="#item"
              />
            ))}
          </div>
        )
      },
      {
        id: 'prices',
        title: 'Prices, discount and actions',
        render: () => (
          <div className={layout.cards}>
            <ShopCard
              name="On sale"
              image={covers.emote}
              rarity="epic"
              discount="-20%"
              price={<ShopCardPrice kind="mana" amount="45" network="polygon" />}
              actions={<CardActionButton aria-label="Add to favourites" />}
              href="#item"
            />
            <ShopCard
              name="Ethereum item"
              image={covers.wearable}
              rarity="legendary"
              price={<ShopCardPrice kind="mana" amount="1,200" network="ethereum" />}
              href="#item"
            />
            <ShopCard
              name="Not for sale"
              placeholder={<BackpackUpperBody size={48} aria-hidden="true" />}
              price={<ShopCardPrice kind="none" label="Not for sale" />}
              href="#item"
            />
          </div>
        )
      },
      {
        id: 'catalogue',
        title: 'Catalogue variant and toggle selection',
        render: () => (
          <div className={layout.cards}>
            <ShopCard
              variant="catalogue"
              name="Catalogue gloves"
              image={covers.wearable}
              rarity="uncommon"
              creator={people.grace}
              price={<ShopCardPrice kind="credits" amount="1,234,567" />}
              href="#item"
            />
            <SelectableShopCard />
          </div>
        )
      },
      {
        id: 'skeleton',
        title: 'ShopCardSkeleton',
        render: () => (
          <div className={layout.cards}>
            <ShopCardSkeleton />
            <ShopCardSkeleton />
          </div>
        )
      }
    ]
  },
  {
    name: 'WearableCard',
    group: 'Cards',
    summary:
      'An item tile of the Explorer HUD backpack: its thumbnail on its rarity, an equipped ring, and an action on hover.',
    components: ['WearableCard'],
    examples: [
      {
        id: 'rarities',
        title: 'Every rarity and none, drawn without a host picture',
        note: 'Point at a card for its lift and its EQUIP action. A host passes its own tile, flap, ring and panel pictures.',
        render: () => (
          <div className={layout.tiles}>
            {itemRarities.map((rarity) => (
              <WearableCard
                key={rarity}
                name={`${titled(rarity)} gloves`}
                thumbnail={covers.wearable}
                rarity={rarity}
                categoryIcon={glyph}
              />
            ))}
          </div>
        )
      },
      {
        id: 'states',
        title:
          'Equipped, selected, new, smart, in an emote slot, incompatible, one that cannot be taken off, and without a picture',
        note: 'The incompatible card says why on hover. A missing thumbnail leaves the tile alone.',
        render: () => (
          <div className={layout.tiles}>
            <WearableCard
              name="Equipped gloves"
              thumbnail={covers.wearable}
              rarity="legendary"
              categoryIcon={glyph}
              equipped
            />
            <WearableCard
              name="Selected gloves"
              thumbnail={covers.wearable}
              rarity="epic"
              categoryIcon={glyph}
              selected
            />
            <WearableCard
              name="New gloves"
              thumbnail={covers.wearable}
              rarity="uncommon"
              categoryIcon={glyph}
              isNew
            />
            <WearableCard
              name="Smart gloves"
              thumbnail={covers.wearable}
              rarity="mythic"
              categoryIcon={glyph}
              isSmart
            />
            <WearableCard
              name="Unicycle"
              thumbnail={covers.emote}
              rarity="rare"
              categoryIcon={glyph}
              equipped
              slotNumber={3}
            />
            <WearableCard
              name="Incompatible gloves"
              thumbnail={covers.wearable}
              rarity="common"
              categoryIcon={glyph}
              incompatible
            />
            <WearableCard
              name="Body shape"
              thumbnail={covers.wearable}
              categoryIcon={glyph}
              equipped
              unequippable={false}
            />
            <WearableCard name="No picture" rarity="unique" categoryIcon={glyph} />
          </div>
        )
      },
      {
        id: 'pick',
        title: 'Tiles to pick: a count, the chosen one ringed, and one a note covers',
        note: 'Press a tile to choose it. The covered one takes no press.',
        render: () => <SelectableWearableCards />
      }
    ]
  },
  {
    name: 'EquippedItemCard',
    group: 'Cards',
    summary:
      'An item a profile shows: its thumbnail on its rarity, its name, a rarity tag and a link to its sale.',
    components: ['EquippedItemCard'],
    examples: [
      {
        id: 'rarities',
        title: 'Every rarity, none, and one the foundations do not colour',
        note: 'Every other card is on sale: point at it for its frame and its link.',
        render: () => (
          <div className={layout.itemCards}>
            {[...itemRarities.slice(0, 8), undefined, 'golden'].map((rarity, index) => (
              <EquippedItemCard
                key={rarity ?? 'none'}
                name={rarity ? `${titled(rarity)} gloves` : 'Plain gloves'}
                thumbnail={covers.wearable}
                rarity={rarity}
                shopUrl={index % 2 === 0 ? '#item' : undefined}
                categoryIcon={glyph}
              />
            ))}
          </div>
        )
      },
      {
        id: 'pinned',
        title: 'The link kept in the card, with a price, a long name and no picture',
        render: () => (
          <div className={layout.itemCards}>
            <EquippedItemCard
              name="Epic gloves"
              thumbnail={covers.wearable}
              rarity="epic"
              shopUrl="#item"
              shopLabel="Buy"
              price="25 Credits"
              pinShop
              categoryIcon={glyph}
            />
            <EquippedItemCard
              name="A pair of gloves whose name is far too long for one line"
              thumbnail={covers.wearable}
              rarity="legendary"
              shopUrl="#item"
              shopLabel="View"
              pinShop
              categoryIcon={glyph}
            />
            <EquippedItemCard name="No picture" rarity="rare" shopUrl="#item" pinShop />
            <EquippedItemCard rarity="unique" price="120 MANA" />
          </div>
        )
      },
      {
        id: 'action',
        title: 'With an action: the picture is a button, and a click anywhere on the card runs it',
        render: () => (
          <div className={layout.itemCards}>
            <EquippedItemCard
              name="Disco"
              thumbnail={covers.wearable}
              rarity="epic"
              categoryIcon={glyph}
              onAction={() => undefined}
              actionLabel="Play Disco"
            />
            <EquippedItemCard
              name="Disco, on sale"
              thumbnail={covers.wearable}
              rarity="rare"
              shopUrl="#item"
              categoryIcon={glyph}
              onAction={() => undefined}
              actionLabel="Play Disco, on sale"
            />
          </div>
        )
      }
    ]
  },
  {
    name: 'BlogCard',
    group: 'Cards',
    summary: 'A blog post in a list, standard or related.',
    components: ['BlogCard'],
    examples: [
      {
        id: 'index',
        title: 'Blog index grid',
        note: "In the Blog index's grid: three columns up to 1312px, two below 1024px, one below 768px. Hover a card with a pointer at 768px or wider.",
        render: () => (
          <ul className={layout.blogIndex}>
            {blogPosts.map((post) => (
              <li key={post.title}>
                <BlogCard
                  title={post.title}
                  href="#post"
                  cover={<img src={post.cover} alt="" />}
                  category={post.category}
                  metadata={post.date}
                  author={people.ada.name}
                />
              </li>
            ))}
          </ul>
        )
      },
      {
        id: 'related',
        title: 'Related posts',
        note: 'variant="related" in an article\'s related grid, up to 1180px.',
        render: () => (
          <ul className={layout.blogRelated}>
            {blogPosts.map((post) => (
              <li key={post.title}>
                <BlogCard
                  variant="related"
                  title={post.title}
                  href="#post"
                  cover={<img src={post.cover} alt="" />}
                  category={post.category}
                  metadata={post.date}
                />
              </li>
            ))}
          </ul>
        )
      },
      {
        id: 'no-cover',
        title: 'Without a cover',
        render: () => (
          <ul className={layout.blogIndex}>
            <li>
              <BlogCard title="Release notes" href="#post" metadata="5 min read" />
            </li>
          </ul>
        )
      },
      {
        id: 'loading',
        title: 'Loading',
        note: 'loading draws a busy skeleton sized for a post with a two-line title and a 24px category badge, as Sites cards carry; the loaded card beside it has that content, so the two match.',
        render: () => (
          <div className={layout.blogGrids}>
            <ul className={layout.blogIndex}>
              <li>
                <BlogCard
                  title={loadedTitle}
                  href="#post"
                  cover={<img src={blogPosts[0]!.cover} alt="" />}
                  category={<span className={layout.blogBadge}>Announcements</span>}
                  metadata={blogPosts[0]!.date}
                  author={people.ada.name}
                />
              </li>
              <li>
                <BlogCard loading loadingLabel="Loading a post" />
              </li>
              <li>
                <BlogCard loading loadingLabel="Loading a post" />
              </li>
            </ul>
            <ul className={layout.blogRelated}>
              <li>
                <BlogCard
                  variant="related"
                  title={loadedTitle}
                  href="#post"
                  cover={<img src={blogPosts[1]!.cover} alt="" />}
                  category={<span className={layout.blogBadge}>Guides</span>}
                  metadata={blogPosts[1]!.date}
                />
              </li>
              <li>
                <BlogCard variant="related" loading loadingLabel="Loading a related post" />
              </li>
              <li>
                <BlogCard variant="related" loading loadingLabel="Loading a related post" />
              </li>
            </ul>
          </div>
        )
      },
      {
        id: 'focus',
        title: 'Hover and keyboard focus',
        note: 'At 768px or wider, hover or keyboard focus raises the card, strengthens its border and underlines the title. Focus the card shows it through keyboard focus, without a pointer.',
        render: () => <FocusedBlogCard />
      }
    ]
  },
  {
    name: 'BlogBanner',
    group: 'Cards',
    summary: 'The featured post banner.',
    components: ['BlogBanner'],
    examples: [
      {
        id: 'banner',
        title: 'Featured post',
        render: () => (
          <BlogBanner
            title="Building the open metaverse"
            href="#post"
            cover={<img src={covers.festival} alt="" />}
            category="Featured"
            metadata="Jan 2, 2026"
            summary="How the community shapes what comes next."
            actionLabel="Read more"
          />
        )
      },
      {
        id: 'minimal',
        title: 'Without cover or summary',
        render: () => <BlogBanner title="Short banner" href="#post" actionLabel="Read" />
      }
    ]
  }
]
