import { siteUrl } from "../../data/site";
import type { CSSProperties, ReactNode } from "react";
import { useState, useSyncExternalStore } from "react";
import { Button, CardActionButton, Tabs, TabsBar } from "@dcl/one-ui/components";
import { SitesChromeMaybe } from "../frames/SitesChrome";
import { asset } from "../../asset";
import CoverImage from "../../components/CoverImage";
import "./stwhatson.css";

/** A poster at its source URL, as the reference's event cards show it; the landing keeps its resized copy. */
function Poster({ className, hue, image, label, priority = false, children }: {
  className: string;
  hue: number;
  image?: string | null;
  label: string;
  priority?: boolean;
  children?: ReactNode;
}) {
  const gradient = `linear-gradient(150deg, hsl(${hue} 70% 52%) 0%, hsl(${(hue + 40) % 360} 60% 28%) 100%)`;
  // The hue gradient stands in for a missing or failed image only: under a loaded image it would show through
  // the image's translucent pixels, which the reference draws over the card alone.
  return (
    <div className={`${className} wo-poster`} role="img" aria-label={label} style={{ "--hue": hue, ...(image ? {} : { backgroundImage: gradient }) } as CSSProperties}>
      {image && <CoverImage className="wo-poster__image" src={image} alt="" fallback={<span className="wo-poster__fallback" style={{ backgroundImage: gradient }} />}
        loading={priority ? "eager" : "lazy"} fetchPriority={priority ? "high" : undefined} decoding="async" />}
      {children}
    </div>
  );
}

/** The creator's avatar: a face image when a profile has one, over the profile's colour; else the hue circle. */
type WoFace = {
  face?: string | null;
  color?: string;
};

export type WoLiveCard = WoFace & {
  id: string;
  title: string;
  users: number | null;
  isEvent: boolean;
  creator: string;
  hue: number;
  image?: string | null;
  jumpUrl?: string;
};

export type WoUpcomingCard = WoFace & {
  id: string;
  name: string;
  creator: string;
  time: string;
  hue: number;
  image?: string | null;
  jumpUrl?: string;
  attending?: boolean;
};

export type WoDayEvent = WoFace & {
  id: string;
  name: string;
  creator: string;
  time: string;
  /** The label of the small card that lists the day on a phone: "Today 6:00 PM". */
  upcomingTime?: string;
  live: boolean;
  users?: number;
  x?: number;
  y?: number;
  hue: number;
  image?: string | null;
  attending?: boolean;
};

function CreatorAvatar({ className, size, hue, face, color }: WoFace & { className: string; size: string; hue: number }) {
  const style = { "--sz": size, "--hue": hue, ...(color ? { background: color } : {}) } as CSSProperties;
  return (
    <span className={`${className} u-avatar`} style={style} aria-hidden="true">
      {face ? <img className="u-avatar__img" src={face} alt="" /> : null}
    </span>
  );
}

// The icons carry the test ids of the reference's Material icons, so a parity capture pairs them.
const SensorsIcon = ({ size = 24 }: { size?: number }) => (
  <svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" aria-hidden="true" data-testid="SensorsRoundedIcon">
    <path d="M8.54 8.54c.35.35.37.88.1 1.29C8.24 10.45 8 11.2 8 12c0 .8.24 1.55.64 2.17.27.41.24.95-.11 1.29-.43.43-1.17.4-1.51-.11C6.38 14.4 6 13.24 6 12c0-1.21.36-2.33.97-3.28.36-.54 1.11-.64 1.57-.18m6.92 6.92c.43.43 1.17.4 1.51-.11C17.62 14.4 18 13.24 18 12c0-1.24-.38-2.4-1.03-3.36-.34-.5-1.08-.54-1.51-.11-.35.35-.37.88-.11 1.29.41.63.65 1.38.65 2.18 0 .8-.24 1.55-.64 2.17-.27.41-.24.95.1 1.29M12 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2m6.32 8.32c.42.42 1.12.39 1.5-.08C21.18 16.53 22 14.36 22 12s-.82-4.53-2.18-6.24c-.37-.47-1.07-.5-1.5-.08-.36.36-.4.92-.08 1.32 1.1 1.37 1.76 3.11 1.76 5s-.66 3.63-1.76 5c-.32.39-.28.96.08 1.32M5.68 5.68c-.42-.42-1.12-.39-1.5.08C2.82 7.47 2 9.64 2 12s.82 4.53 2.18 6.24c.37.47 1.07.5 1.5.08.36-.36.4-.92.08-1.32C4.66 15.63 4 13.89 4 12s.66-3.63 1.76-5c.32-.39.28-.96-.08-1.32" />
  </svg>
);
const PersonIcon = () => (
  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true" data-testid="PermIdentityRoundedIcon">
    <path d="M12 6c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2 .9-2 2-2m0 9c2.7 0 5.8 1.29 6 2v1H6v-.99c.2-.72 3.3-2.01 6-2.01m0-11C9.79 4 8 5.79 8 8s1.79 4 4 4 4-1.79 4-4-1.79-4-4-4Zm0 9c-2.67 0-8 1.34-8 4v2c0 .55.45 1 1 1h14c.55 0 1-.45 1-1v-2c0-2.66-5.33-4-8-4Z" />
  </svg>
);
const BELL_BODY = "M5.4167 9.44547C5.44685 7.63928 6.21335 5.91687 7.55087 4.64979C8.88839 3.38271 10.6897 2.6725 12.5663 2.67236C14.4429 2.67223 16.2444 3.38217 17.5821 4.64907C18.9198 5.91596 19.6866 7.63825 19.717 9.44444L19.7173 12.2663L21.4284 15.9735C21.4888 16.1045 21.5143 16.2479 21.5025 16.3907C21.4907 16.5335 21.4421 16.6712 21.361 16.7914C21.2799 16.9115 21.169 17.0103 21.0382 17.0787C20.9074 17.1471 20.7609 17.183 20.6121 17.1831L15.3891 17.1835L15.3919 17.1891L9.73956 17.1894L9.74313 17.1835L4.52097 17.1824C4.37215 17.1824 4.2257 17.1465 4.0949 17.0782C3.96411 17.0099 3.85311 16.9112 3.77197 16.7911C3.69084 16.671 3.64213 16.5333 3.63028 16.3905C3.61843 16.2477 3.6438 16.1043 3.7041 15.9733L5.41606 12.2689L5.41581 9.44696L5.4167 9.44547ZM7.20443 9.44545L7.20361 12.4516C7.20351 12.5717 7.17727 12.6905 7.1266 12.8002L5.89689 15.4632L19.2368 15.4629L18.006 12.7997C17.9557 12.69 17.9298 12.5713 17.93 12.4514L17.9284 9.44595C17.9026 8.09324 17.3262 6.80431 16.3233 5.85649C15.3203 4.90867 13.971 4.37771 12.5655 4.37784C11.16 4.37797 9.81067 4.90918 8.80791 5.85719C7.80515 6.80519 7.22904 8.09422 7.20354 9.44694L7.20443 9.44545Z";
const BELL_CLAPPER = "M9.48535 18.7616C9.59625 19.475 9.96951 20.1265 10.5372 20.5974C11.1049 21.0683 11.8294 21.3274 12.5788 21.3275C13.3282 21.3277 14.0528 21.0689 14.6207 20.5982C15.1886 20.1276 15.5622 19.4763 15.6734 18.7629L9.48535 18.7616Z";
const BELL_ACTIVE = "M15.6737 18.7625C15.5625 19.4758 15.1888 20.1277 14.621 20.5984C14.0531 21.069 13.3282 21.328 12.579 21.3279C11.8296 21.3277 11.1047 21.0683 10.537 20.5974C9.96927 20.1265 9.59613 19.4749 9.48522 18.7615L15.6737 18.7625ZM12.5663 2.67261C14.4428 2.67251 16.2442 3.38232 17.5819 4.64917C17.9225 4.97176 18.2253 5.32444 18.4891 5.69995L16.4413 7.74976L16.1805 7.48901C15.0869 6.3954 13.3142 6.39538 12.2206 7.48901C11.1271 8.58265 11.127 10.3554 12.2206 11.449L14.4608 13.6892C14.9859 14.2141 15.6988 14.5095 16.4413 14.5095C17.1837 14.5093 17.8958 14.2142 18.4208 13.6892L19.7567 12.3523L21.4286 15.9734C21.4889 16.1042 21.5135 16.2477 21.5018 16.3904C21.4901 16.5332 21.4423 16.6716 21.3612 16.7917C21.2801 16.9119 21.1687 17.0105 21.038 17.0789C20.9073 17.1472 20.7609 17.1832 20.6122 17.1833H15.3885L15.3915 17.1892H9.73913L9.74304 17.1833L4.52038 17.1824C4.37169 17.1823 4.22528 17.1462 4.0946 17.0779C3.96389 17.0095 3.85245 16.9108 3.77136 16.7908C3.69038 16.6708 3.64159 16.533 3.62975 16.3904C3.61793 16.2476 3.6437 16.1043 3.70397 15.9734L5.41589 12.2693V9.44702L5.41686 9.44507C5.44712 7.63911 6.21335 5.91712 7.55065 4.65015C8.88817 3.38306 10.6897 2.67274 12.5663 2.67261Z";
const BELL_CHECK = "M20.0042 6.3392C20.4417 5.90174 21.1508 5.90174 21.5882 6.3392C22.0257 6.77666 22.0257 7.48574 21.5882 7.9232L17.1075 12.4039C16.8974 12.614 16.6126 12.7321 16.3155 12.7321C16.0184 12.7321 15.7336 12.614 15.5235 12.4039L13.2832 10.1635C12.8457 9.72609 12.8457 9.01701 13.2832 8.57955C13.7206 8.14209 14.4297 8.14209 14.8672 8.57955L16.3155 10.0279L20.0042 6.3392Z";
/** The reference's remind-me bell: an outline, or filled with a check once the player attends. */
/** The reference small card (decentraland-ui2 EventSmallCard) paints a faceless avatar in the theme's success.dark. */
const SMALL_CARD_FALLBACK = "#00B453";
const RemindIcon = ({ active }: { active: boolean }) => (
  <svg viewBox="0 0 25 25" width="16" height="16" fill="none" aria-hidden="true">
    {active ? <><path d={BELL_ACTIVE} fill="#FF2D55" /><path d={BELL_CHECK} fill="#FF2D55" /></> : <><path d={BELL_BODY} fill="currentColor" /><path d={BELL_CLAPPER} fill="currentColor" /></>}
  </svg>
);
const JumpInIcon = () => (
  <svg viewBox="0 0 24 24" width="24" height="24" fill="none" aria-hidden="true">
    <rect x="0.75" y="0.75" width="22.5" height="22.5" rx="5.25" stroke="#FCFCFC" strokeOpacity="0.3" strokeWidth="1.5" />
    <path fill="#FCFCFC" d="M18.7111 11.065L14.034 6.39027C13.2002 5.55695 11.7971 6.14637 11.7971 7.32523V8.86994C11.7564 8.86994 11.7361 8.86994 11.6954 8.86994H7.25895C6.50654 8.86994 5.89648 9.45936 5.89648 10.2114V13.7683C5.89648 14.5203 6.50654 15.1301 7.25895 15.1301H11.6751C11.7158 15.1301 11.7361 15.1301 11.7768 15.1301V16.6748C11.7768 17.8536 13.2002 18.4431 14.0137 17.6097L18.6908 12.935C19.2195 12.4065 19.2195 11.5732 18.7111 11.065Z" />
  </svg>
);
const CalendarIcon = () => (
  <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" aria-hidden="true" data-testid="CalendarMonthIcon">
    <path d="M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20a2 2 0 0 0 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2Zm0 16H5V10h14v10ZM9 14H7v-2h2v2Zm4 0h-2v-2h2v2Zm4 0h-2v-2h2v2Zm-8 4H7v-2h2v2Zm4 0h-2v-2h2v2Zm4 0h-2v-2h2v2Z" />
  </svg>
);
const ChevronLeft = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true" data-testid="ChevronLeftIcon">
    <path d="M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12l4.58-4.59Z" />
  </svg>
);
const ChevronRight = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true" data-testid="ChevronRightIcon">
    <path d="M8.59 16.59 13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41Z" />
  </svg>
);
const CopyIcon = () => (
  <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" aria-hidden="true">
    <path d="M16 1H4a2 2 0 0 0-2 2v14h2V3h12V1Zm3 4H8a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2Zm0 16H8V7h11v14Z" />
  </svg>
);
const CheckIcon = () => (
  <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" aria-hidden="true" data-testid="CheckIcon">
    <path d="M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17Z" />
  </svg>
);

const AccessTimeIcon = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true" data-testid="AccessTimeIcon">
    {/* Two paths, as MUI AccessTime: the hand in the ring's path rasterises its edge pixels differently. */}
    <path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2M12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8" />
    <path d="M12.5 7H11v6l5.25 3.15.75-1.23-4.5-2.67z" />
  </svg>
);
const AddIcon = () => (
  <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true" data-testid="AddIcon">
    <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2Z" />
  </svg>
);

const eventHref = (id: string) => `/whats-on/${encodeURIComponent(id)}`;

function copyEventLink(id: string) {
  try {
    void navigator.clipboard?.writeText(`${window.location.origin}${eventHref(id)}`);
  } catch {
    // The clipboard is optional: the link stays on the card.
  }
}

function Dots({ count, className }: { count: number; className: string }) {
  if (count < 2) return null;
  return (
    <div className={`wo-dots ${className}`} aria-hidden="true">
      {Array.from({ length: count }, (_, i) => <span key={i} className={"wo-dots__dot" + (i === 0 ? " is-active" : "")} />)}
    </div>
  );
}

function Badges({ live, users }: { live: boolean; users?: number | null }) {
  return (
    <span className="wo-badges">
      {live && <span className="wo-badge wo-badge--live"><SensorsIcon size={14} />LIVE</span>}
      {users != null && <span className="wo-badge wo-badge--users"><span className="wo-badge__dot" /><PersonIcon />{users}</span>}
    </span>
  );
}

function LiveNowCard({ card, priority }: { card: WoLiveCard; priority: boolean }) {
  return (
    <article className="wo-ln__card">
      <a className="wo-ln__area" href={card.jumpUrl ?? siteUrl("/play/")} target="_blank" rel="noreferrer">
        <Poster className="wo-ln__media" hue={card.hue} image={card.image} label={card.title} priority={priority}>
          <div className="wo-ln__badges"><Badges live={card.isEvent} users={card.users} /></div>
        </Poster>
        <div className="wo-ln__body">
          <div className="wo-ln__info">
            <h3 className="wo-ln__title">{card.title}</h3>
            <div className="wo-ln__creatorrow">
              <CreatorAvatar className="wo-ln__avatar" size="32px" hue={card.hue} face={card.face} color={card.color} />
              <span className="wo-ln__creator">{"by "}<strong>{card.creator}</strong></span>
            </div>
          </div>
          <div className="wo-ln__jumpwrap">
            <span className="wo-ln__jump">
              <span>JUMP IN</span>
              <JumpInIcon />
            </span>
          </div>
        </div>
      </a>
    </article>
  );
}

function LiveNow({ cards }: { cards: WoLiveCard[] }) {
  if (!cards.length) return null;
  return (
    <section className="wo-ln">
      <header className="wo-ln__header">
        <span className="wo-ln__sensors" aria-hidden="true"><SensorsIcon /></span>
        <h2 className="wo-ln__heading">Live Now</h2>
      </header>
      <div className="wo-ln__grid">
        {cards.map((c, index) => <LiveNowCard key={c.id} card={c} priority={index === 0} />)}
      </div>
      <Dots count={cards.length} className="wo-ln__dots" />
    </section>
  );
}

function SmallCard({ event, priority, signedIn = false }: { event: WoUpcomingCard; priority: boolean; signedIn?: boolean }) {
  return (
    <div className="wo-sm">
      <a className="wo-card__overlay" href={eventHref(event.id)} aria-label={event.name} />
      <Poster className="wo-sm__thumbwrap" hue={event.hue} image={event.image} label={event.name} priority={priority} />
      <div className="wo-sm__text">
        <div className="wo-sm__top">
          <div className="wo-sm__titlerow">
            <h3 className="wo-sm__title">{event.name}</h3>
            {signedIn
              ? <a className="wo-sm__mobileaction" href={eventHref(event.id)} aria-label="Remind me"><RemindIcon active={!!event.attending} /></a>
              : <a className="wo-sm__mobileaction" href={eventHref(event.id)} aria-label="Add to calendar"><CalendarIcon /></a>}
          </div>
          <div className="wo-sm__creatorrow">
            <CreatorAvatar className="wo-sm__avatar" size="19px" hue={event.hue} face={event.face} color={event.face ? "transparent" : SMALL_CARD_FALLBACK} />
            <span className="wo-sm__creator">by{" "}<span className="wo-sm__creatorhi">{event.creator}</span></span>
          </div>
        </div>
        <div className="wo-sm__pill" data-role="time-pill">
          <AccessTimeIcon />
          <span className="wo-sm__time">{event.time}</span>
        </div>
        <div className="wo-sm__actions" data-role="hover-actions">
          <Button variant="secondary" size="sm" className="wo-sm__textbtn" href={event.jumpUrl ?? siteUrl("/play/")} target="_blank" rel="noreferrer" leadingIcon={<JumpInIcon />}>
            Jump in to world
          </Button>
          <CardActionButton className="wo-sm__iconbtn" aria-label="Copy link" icon={<CopyIcon />} onClick={() => copyEventLink(event.id)} />
        </div>
      </div>
    </div>
  );
}

const UPCOMING_PAGE = 4;

function Upcoming({ events, priority, signedIn }: { events: WoUpcomingCard[]; priority: boolean; signedIn: boolean }) {
  if (!events.length) return null;
  const pages = Array.from({ length: Math.ceil(events.length / UPCOMING_PAGE) }, (_, i) => events.slice(i * UPCOMING_PAGE, (i + 1) * UPCOMING_PAGE));
  return (
    <section className="wo-up">
      <h2 className="wo-up__title">Upcoming</h2>
      <div className="wo-up__grid">
        {events.map((e, index) => <SmallCard key={e.id} event={e} priority={priority && index === 0} signedIn={signedIn} />)}
      </div>
      <div className="wo-up__carousel">
        <div className="wo-up__track">
          {pages.map((page, i) => (
            <div className="wo-up__page" key={i}>
              {page.map((e) => <SmallCard key={e.id} event={e} priority={false} signedIn={signedIn} />)}
            </div>
          ))}
        </div>
        <Dots count={pages.length} className="wo-up__dots" />
      </div>
    </section>
  );
}

function FutureCard({ event }: { event: WoDayEvent }) {
  return (
    <div className="wo-fc">
      <a className="wo-card__overlay" href={eventHref(event.id)} aria-label={event.name} />
      {event.image && <Poster className="wo-fc__imgwrap" hue={event.hue} image={event.image} label={event.name} />}
      <div className="wo-fc__content">
        <h3 className="wo-fc__title">{event.name}</h3>
        <div className="wo-fc__creatorrow" data-role="creator-row">
          <CreatorAvatar className="wo-fc__avatar" size="19px" hue={event.hue} face={event.face} color={event.color} />
          <span className="wo-fc__creator">{"By "}<span className="wo-fc__creatorhi">{event.creator}</span></span>
        </div>
        <div className="wo-fc__pill" data-role="time-pill">
          <AccessTimeIcon />
          <span className="wo-fc__time">{event.time}</span>
        </div>
        <div className="wo-fc__actions" data-role="hover-actions">
          <Button variant="secondary" size="sm" className="wo-fc__textbtn" href={eventHref(event.id)} leadingIcon={<JumpInIcon />}>
            View event
          </Button>
          <CardActionButton className="wo-fc__iconbtn" aria-label="Copy link" icon={<CopyIcon />} onClick={() => copyEventLink(event.id)} />
        </div>
      </div>
    </div>
  );
}

function LiveCard({ event }: { event: WoDayEvent }) {
  return (
    <div className="wo-lc">
      <a className="wo-card__overlay" href={eventHref(event.id)} aria-label={event.name} />
      <Poster className="wo-lc__media" hue={event.hue} image={event.image} label={event.name}>
        <div className="wo-lc__badges"><Badges live users={event.users} /></div>
      </Poster>
      <div className="wo-lc__content">
        <h3 className="wo-lc__title">{event.name}</h3>
      </div>
    </div>
  );
}

function DayColumn({ events, label }: { events: WoDayEvent[]; label: string }) {
  return (
    <div className="wo-col" role="list" aria-label={label}>
      <div className="wo-col__scroll">
        {events.map((e) => (
          <div className="wo-col__slot" role="listitem" key={`${e.id}-${e.time}`}>
            {e.live ? <LiveCard event={e} /> : <FutureCard event={e} />}
          </div>
        ))}
        <span className="wo-col__filler" aria-hidden="true" />
      </div>
    </div>
  );
}

function HostBanner() {
  const CHECKS = [
    "Choose a vibe people can drop into.",
    "Add the event details.",
    "Publish it and send the invites.",
  ];
  return (
    <section className="wo-host" aria-label="Host an Event">
      <img className="wo-host__avatar" src={asset("assets/whatson-host-avatar.webp")} alt="" aria-hidden="true" />
      <div className="wo-host__content">
        <h3 className="wo-host__title">Host an Event</h3>
        <p className="wo-host__subtitle">Bring people together, your way.</p>
        <div className="wo-host__checks">
          {CHECKS.map((c) => (
            <div className="wo-host__check" key={c}>
              <span className="wo-host__box" aria-hidden="true"><CheckIcon /></span>
              <span className="wo-host__checktext">{c}</span>
            </div>
          ))}
        </div>
        <div className="wo-host__buttons">
          <Button variant="primary" size="md" className="wo-host__btn wo-host__btn--primary" href="/landings/submit-hangout" data-testid="create-button" leadingIcon={<AddIcon />}>
            Create an Event
          </Button>
          <Button variant="secondary" size="md" className="wo-host__btn wo-host__btn--secondary" href="/docs/player/faqs/posting-events/" data-testid="learn-more-button">
            Learn More
          </Button>
        </div>
      </div>
      <div className="wo-host__scene" aria-hidden="true">
        <img className="wo-host__sceneimg" src={asset("assets/whatson-host-scene.webp")} alt="" />
      </div>
    </section>
  );
}

/** The most day columns a viewport shows; CSS hides the rest by width (>=1536: 5, >=1200: 4, >=900: 3, >=600: 2, else a list). */
const MAX_COLUMNS = 5;

function visibleColumns(): number {
  if (typeof window === "undefined") return MAX_COLUMNS;
  const w = window.innerWidth;
  return w >= 1536 ? 5 : w >= 1200 ? 4 : w >= 900 ? 3 : w >= 600 ? 2 : 1;
}

const PHONE = "(max-width: 599.98px)";
const subscribePhone = (change: () => void) => {
  const query = window.matchMedia(PHONE);
  query.addEventListener("change", change);
  return () => query.removeEventListener("change", change);
};

/** Whether the viewport is phone width; null while the page is rendered on the server or hydrates. */
function usePhone(): boolean | null {
  return useSyncExternalStore(subscribePhone, () => window.matchMedia(PHONE).matches, () => null);
}

function AllExperiences({ days, dayLabels, signedIn }: { days: WoDayEvent[][]; dayLabels: string[]; signedIn: boolean }) {
  const [start, setStart] = useState(0);
  // The server renders the columns and the phone list, and CSS shows one; once hydrated, only that one stays.
  const phone = usePhone();
  const shown = days.slice(start, start + MAX_COLUMNS);
  const labelFor = (i: number) => dayLabels[i] || `+${i}d`;
  const page = (direction: 1 | -1) => {
    const step = visibleColumns();
    setStart((s) => Math.min(Math.max(0, s + direction * step), Math.max(0, days.length - step)));
  };
  return (
    <section className="wo-all" aria-label="All Events">
      {signedIn ? (
        <TabsBar className="wo-all__tabs" label="All Events" variant="pill">
          <Tabs className="wo-all__title" variant="pill" label="All Events" href="/whats-on" selected />
          <Tabs variant="pill" label="My Events" href="/whats-on?tab=my" />
        </TabsBar>
      ) : (
        <h2 className="wo-all__title">All Events</h2>
      )}
      <nav className="wo-all__nav" aria-label="All Events dates">
        <button type="button" className="wo-all__navbtn wo-all__navbtn--left" aria-label="Navigate to previous dates" disabled={start === 0} onClick={() => page(-1)}>
          <ChevronLeft />
        </button>
        {shown.map((_, i) => (
          <span key={start + i} className={"wo-all__date" + (start + i === 0 ? " is-today" : "")}>{labelFor(start + i)}</span>
        ))}
        <button type="button" className="wo-all__navbtn wo-all__navbtn--right" aria-label="Navigate to next dates" disabled={start + 1 >= days.length} onClick={() => page(1)}>
          <ChevronRight />
        </button>
      </nav>
      {phone !== true && <div className="wo-all__cols">
        {shown.map((events, i) => <DayColumn key={start + i} events={events} label={labelFor(start + i)} />)}
      </div>}
      {phone !== false && <div className="wo-all__list">
        {(shown[0] ?? []).map((e) => (
          <div className="wo-all__slot" key={`${e.id}-${e.time}`}>
            <SmallCard event={{ ...e, time: e.upcomingTime ?? e.time }} priority={false} signedIn={signedIn} />
          </div>
        ))}
      </div>}
      <HostBanner />
    </section>
  );
}

type StWhatSOnProps = {
  chrome?: boolean;
  liveNow?: WoLiveCard[];
  upcoming?: WoUpcomingCard[];
  allDays?: WoDayEvent[][];
  dayLabels?: string[];
  /** Signed in, All Events is a tab beside My Events. */
  signedIn?: boolean;
  notice?: ReactNode;
};

export default function StWhatSOn({
  chrome = true,
  liveNow = [],
  upcoming = [],
  allDays = [],
  dayLabels = [],
  signedIn = false,
  notice = null,
}: StWhatSOnProps) {
  return (
    <SitesChromeMaybe chrome={chrome} active="play" overlayNav>
      <div className="wo">
        <picture>
          <source media="(min-width: 600px)" srcSet={asset("assets/whatson-top.webp")} />
          <img className="wo__bg" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/%3E" alt="" aria-hidden="true" />
        </picture>
        <div className="wo__top" aria-hidden="true" />
        <div className="wo__content">
          {notice}
          <LiveNow cards={liveNow} />
          <Upcoming signedIn={signedIn} events={upcoming} priority={liveNow.length === 0} />
          <AllExperiences days={allDays} dayLabels={dayLabels} signedIn={signedIn} />
        </div>
      </div>
    </SitesChromeMaybe>
  );
}
