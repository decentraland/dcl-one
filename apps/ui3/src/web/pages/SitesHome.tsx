import { siteUrl } from "../../data/site";
import SitesChrome from "../frames/SitesChrome";
import { asset } from "../../asset";
import { publicThumbnail } from "../../data/thumbnail";
import CoverImage from "../../components/CoverImage";
import "./siteshome.css";
import { Chevron } from "../../atoms/icons";

type HomeEvent = {
  id: string;
  cat?: string;
  category?: string;
  title: string;
  when?: string;
  grad?: string;
  image?: string | null;
  live?: boolean;
};

const EVENTS: HomeEvent[] = [
  { id: "e1", cat: "MUSIC", title: "Sunset Synthwave \u2014 Live DJ Set on the Pier", when: "Today \u00b7 8:00 PM", grad: "linear-gradient(135deg, #ff2d55 0%, #982de2 70%)", live: true },
  { id: "e2", cat: "COMMUNITY", title: "Metaverse Fashion Week \u2014 Opening Night", when: "Fri \u00b7 6:00 PM", grad: "linear-gradient(135deg, #ff4bed 0%, #6a1fb0 70%)" },
  { id: "e3", cat: "GAMES", title: "Genesis Plaza Trivia Night", when: "Sat \u00b7 7:00 PM", grad: "linear-gradient(135deg, #1f8a70 0%, #438fff 70%)" },
];

type HomeHotspot = {
  id: string;
  title: string;
  online?: number | null;
  image?: string | null;
  grad?: string;
  href?: string;
};

const HOTSPOTS: HomeHotspot[] = [
  { id: "genesis-plaza", title: "Genesis Plaza", grad: "linear-gradient(135deg, #ff743a 0%, #982de2 60%, #1a0a2e 100%)", href: siteUrl("/play/?position=-3%2C-2") },
  { id: "wondermine", title: "WonderMine Crafting Game", grad: "linear-gradient(135deg, #438fff 0%, #ff2d55 60%, #220040 100%)", href: siteUrl("/play/?position=-29%2C55") },
];

type HomeRitual = {
  id: string;
  title: string;
  day?: string | null;
  image?: string | null;
  grad?: string;
  href?: string;
};

const RITUALS: HomeRitual[] = [
  { id: "career-mondays", title: "Career Mondays", grad: "linear-gradient(135deg, #438fff, #2f004d)" },
  { id: "takeover-tuesdays", title: "Takeover Tuesdays", grad: "linear-gradient(135deg, #ff2d55, #350447)" },
  { id: "watch-party-wednesdays", title: "Watch Party Wednesdays", grad: "linear-gradient(135deg, #b05cff, #220040)" },
  { id: "trivia-thursdays", title: "Trivia Thursdays", grad: "linear-gradient(135deg, #1f8a70, #06231c)" },
  { id: "play-with-friends-fridays", title: "Play with Friends Fridays", grad: "linear-gradient(135deg, #ff743a, #3a1500)" },
];

function CardImage({ image, width = 640 }: { image?: string | null; width?: 320 | 640 }) {
  return image ? <CoverImage className="sh__cardimg" src={publicThumbnail(image, width)} fallbackSrc={image} alt="" /> : null;
}
function PlayCta() {
  return <div className="sh__cta"><a className="sh__btn sh__btn--primary" href="/play/">Play</a></div>;
}

type SitesHomeProps = {
  downloads?: string | null;
  events?: HomeEvent[];
  hotspots?: HomeHotspot[];
  rituals?: HomeRitual[];
};

export default function SitesHome({
  events,
  hotspots,
  rituals,
}: SitesHomeProps) {
  const eventCards = events && events.length > 0 ? events : EVENTS;
  const hotspotCards = hotspots && hotspots.length > 0 ? hotspots : HOTSPOTS;
  const ritualCards = rituals && rituals.length > 0 ? rituals : RITUALS;
  return (
    <SitesChrome active="play" overlayNav>
      <div className="sh">
        <section className="sh__hero">
          <img className="sh__heroimg" src={asset("assets/home-hero.webp")} alt="" aria-hidden="true" fetchPriority="high" />
          <div className="sh__heroinner">
            <h1 className="sh__title">Hang Out From Anywhere</h1>
            <p className="sh__sub">Close the Feed. Come Hang Out.</p>
            <PlayCta />
          </div>
        </section>

        <section className="sh__sec sh__whatson">
          <div className="sh__sechead">
            <h2 className="sh__h2">Discover</h2>
            <a className="sh__viewall" href="/whats-on">
              View All <Chevron />
            </a>
          </div>
          <div className="sh__eventgrid">
            {eventCards.map((ev) => (
              <article className="sh__event" key={ev.id}>
                <div
                  className="sh__eventimg"
                  style={{ background: ev.grad }}
                >
                  <CardImage image={ev.image} />
                  {ev.live ? <span className="sh__eventlive">&#x25CF; LIVE</span> : null}
                </div>
                <div className="sh__eventmeta">
                  <span className="sh__eventcat">{ev.cat ?? ev.category}</span>
                  <h3 className="sh__eventtitle">{ev.title}</h3>
                  <span className="sh__eventwhen">{ev.when}</span>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section className="sh__sec sh__vibe">
          <h2 className="sh__h2">Catch the Vibe</h2>
          <div className="sh__viberow">
            {hotspotCards.map((v) => (
              <a className="sh__vibecard" key={v.id} href={v.href ?? siteUrl("/play/")}>
                <div className="sh__vibeimg" style={{ background: v.grad }}>
                  <CardImage image={v.image} />
                  {v.online != null && v.online > 0 ? (
                    <span className="sh__online">&#x25CF; {v.online} ONLINE</span>
                  ) : null}
                </div>
                <div className="sh__vibefoot">
                  <span className="sh__vibeuser">{v.title}</span>
                  <span className="sh__hangout">HANG OUT NOW</span>
                </div>
              </a>
            ))}
          </div>
        </section>

        <section className="sh__sec sh__rituals">
          <h2 className="sh__h2">Your Weekly Rituals</h2>
          <div className="sh__ritualrow">
            {ritualCards.map((r) => (
              <a className="sh__ritual" key={r.id} href={r.href ?? "/whats-on"} style={{ background: r.grad }}>
                <CardImage image={r.image} width={320} />
                {r.day ? <span className="sh__ritualwhen">{r.day}</span> : null}
                <span className="sh__ritualday">{r.title}</span>
              </a>
            ))}
          </div>
        </section>

        <section className="sh__banner">
          <img className="sh__banimg" src={asset("assets/come-hang-out.webp")} alt="" aria-hidden="true" loading="lazy" decoding="async" />
          <div className="sh__bancontent">
            <h2 className="sh__bantitle">Come Hang Out</h2>
            <PlayCta />
          </div>
        </section>
      </div>
    </SitesChrome>
  );
}
