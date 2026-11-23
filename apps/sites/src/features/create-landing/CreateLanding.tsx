import { createContext, useCallback, useContext, useEffect, useId, useMemo, useReducer, useRef, useState, useSyncExternalStore, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { Link } from "react-router";
import { useQuery } from "@tanstack/react-query";

import { docsUrl } from "@web-core/docs";
import { trackLegacyClick, trackSectionViewed, useLegacyTrackScope, type LegacyClick, type LegacySectionViewed, type LegacySession } from "@/core/lib/telemetry/legacy-click";
import { latestPostsQuery, type BlogPostSummary } from "@/data/lib/create-landing/blog-posts";
import { liveScenesQuery, selectLiveScenes, type HotScene } from "@/data/lib/create-landing/live-scenes";

import type { HeroDownloads } from "@/data/lib/download/creator-hub.server";

import { Carousel } from "./Carousel";
import HeroDownload from "./HeroDownload";
import {
  connectCards,
  copy,
  createCards,
  faqItems,
  heroData,
  learnCards,
  whyCards,
  type ConnectCardData,
  type CreateCardData,
} from "./content";
import { useMediaQuery } from "@ui/data/hooks/useMediaQuery";
import discordIcon from "./assets/discord.svg?url";
import dclLogo from "./assets/dcl-logo.svg?url";
import styles from "./CreateLanding.module.css";

/** decentraland.org /create, the Creator Hub's marketing landing. */
type CreateLandingProps = {
  /** The Creator Hub installers for the visitor's system; without them the hero links to the download page. */
  downloads?: HeroDownloads | null;
  /** The visit's session id, for the download events. */
  sid?: string;
  /** The verified wallet, for the download Click's `userId`. */
  userId?: string;
};

export default function CreateLanding({ downloads = null, sid = "", userId = "" }: CreateLandingProps) {
  const session = useMemo(() => ({ sid, userId }), [sid, userId]);
  return (
    <SessionContext.Provider value={session}>
      <main className={styles.landing}>
        <Hero downloads={downloads} sid={sid} userId={userId} />
        <Why />
        <Possibilities />
        <LiveScenes />
        <Connect />
        <Learn />
        <Blog />
        <Faqs />
      </main>
    </SessionContext.Provider>
  );
}

const SessionContext = createContext<Omit<LegacySession, "scope">>({ sid: "" });

/**
 * Sends a control's legacy `Click` (decentraland/sites `useTrackClick` on the reference's `Create/*` sections),
 * deferred in this component's scope as the legacy hook's queue was.
 */
function useClick(): (click: LegacyClick) => void {
  const session = useContext(SessionContext);
  const scope = useLegacyTrackScope();
  return useCallback((click: LegacyClick) => trackLegacyClick(click, { ...session, scope }), [session, scope]);
}

/**
 * The legacy `Section Viewed`: once, the first time a tenth of the section is in the viewport (the reference's
 * `AnimatedSection`, `react-intersection-observer` with `triggerOnce` and threshold 0.1). A browser without
 * IntersectionObserver sends nothing, as the reference could not either.
 */
function useSectionViewed(section: LegacySectionViewed) {
  const session = useContext(SessionContext);
  const scope = useLegacyTrackScope();
  const latest = useRef({ ...session, scope });
  latest.current = { ...session, scope };
  const sent = useRef(false);
  const observer = useRef<IntersectionObserver | null>(null);
  return useCallback(
    (node: HTMLElement | null) => {
      observer.current?.disconnect();
      observer.current = null;
      if (!node || sent.current || typeof IntersectionObserver === "undefined") return;
      const watch = new IntersectionObserver(
        (entries) => {
          if (sent.current || !entries.some((entry) => entry.isIntersecting && entry.intersectionRatio >= SECTION_THRESHOLD)) return;
          sent.current = true;
          watch.disconnect();
          trackSectionViewed(section, latest.current);
        },
        { threshold: SECTION_THRESHOLD },
      );
      watch.observe(node);
      observer.current = watch;
    },
    [section],
  );
}

const SECTION_THRESHOLD = 0.1;

/** A section heading with one highlighted run. `title` follows the highlight after its own space text node, as the
 * reference writes `<span>{highlight}</span> {title}`; `before`/`after` join the highlight with no extra node. */
function Title({ highlight, title, before = "", after = "" }: { highlight: string; title?: string; before?: string; after?: string }) {
  return (
    <h2 className={styles.sectionTitle}>
      {before}
      <span>{highlight}</span>
      {title === undefined ? after : <>{" "}{title}</>}
    </h2>
  );
}

const TYPE_DELAY = 100;
type Typing = { letter: number; word: number; erase: boolean };

/** Types each word a letter at a time, rests on it, then erases it. `still` shows the first word and runs no timer. */
function useTypingWord(words: readonly string[], still: boolean): string {
  const [state, tick] = useReducer((s: Typing): Typing => {
    const length = words[s.word]!.length;
    if (!s.erase) return s.letter === length - 1 ? { ...s, erase: true } : { ...s, letter: s.letter + 1 };
    if (s.letter > 0) return { ...s, letter: s.letter - 1 };
    return { letter: 0, word: (s.word + 1) % words.length, erase: false };
  }, { letter: 0, word: 0, erase: false });
  const atEnd = state.letter === words[state.word]!.length - 1;
  useEffect(() => {
    if (still) return;
    const id = setTimeout(tick, atEnd ? TYPE_DELAY * 10 : TYPE_DELAY);
    return () => clearTimeout(id);
  }, [state, atEnd, still]);
  return still ? words[0]! : words[state.word]!.slice(0, state.letter + 1);
}

function ViewAll({ to, children, onClick }: { to: string; children: string; onClick: () => void }) {
  return (
    <Link className={styles.viewAll} to={to} onClick={onClick}>
      {children}
      <svg className={styles.chevronRight} viewBox="0 0 24 24" aria-hidden="true" focusable="false" data-testid="ChevronRightIcon">
        <path d="M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z" />
      </svg>
    </Link>
  );
}

const noSubscribe = () => () => {};

/** False on the server and during hydration, true from the first client render after it. */
function useHydrated(): boolean {
  return useSyncExternalStore(noSubscribe, () => true, () => false);
}

function Hero({ downloads, sid, userId }: Required<CreateLandingProps>) {
  const click = useClick();
  const isMobile = useMediaQuery("(max-width: 767px)");
  // The server cannot know the viewport: the video mounts after hydration, when isMobile is known, so each
  // viewport requests only its own file (as the reference's client-rendered Hero.tsx does).
  const hydrated = useHydrated();
  // A visitor who asks for reduced motion gets the poster and one word: no video playback and no typing loop.
  const reduceMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  const word = useTypingWord(heroData.changingWords, reduceMotion);
  const video = isMobile ? heroData.videoPortrait : heroData.videoLandscape;
  const poster = isMobile ? heroData.imagePortrait : heroData.imageLandscape;
  return (
    <>
      <section className={styles.hero}>
        <div className={styles.heroBg}>
          {hydrated && <video key={`${video.url}:${reduceMotion}`} loop muted autoPlay={!reduceMotion} playsInline width={video.width} height={video.height} poster={poster.url} src={video.url} />}
        </div>
        <div className={styles.heroContent}>
          <h1 className={styles.heroTitle}>
            {heroData.titleFirstLine}
            <br />
            <span>{word}</span>
            <br />
            {heroData.titleLastLine}
          </h1>
          <p className={styles.heroSubtitle}>{heroData.subtitle}</p>
          <div className={styles.heroActions}>
            <HeroDownload downloads={downloads} sid={sid} userId={userId} />
            <a className={`${styles.cta} ${styles.ctaMobile}`} href={docsUrl("creator")} onClick={() => click({ place: "Creators Hero" })}>
              <span className={styles.ctaLabel}>{copy.heroMobileCta}</span>
            </a>
          </div>
        </div>
      </section>
      <button type="button" className={styles.scroll} aria-label={copy.heroScrollLabel} onClick={() => {
        click({ place: "Creators Hero", title: "scroll-to-why" });
        window.scrollBy({ top: window.innerHeight, behavior: "smooth" });
      }}>
        <svg className={styles.chevron} xmlns="http://www.w3.org/2000/svg" width="21" height="14" viewBox="0 0 21 14" aria-hidden="true">
          <path fillRule="evenodd" clipRule="evenodd" d="M0.585815 3.41424L3.41424 0.585815L10.5 7.6716L17.5858 0.585815L20.4142 3.41424L10.5 13.3285L0.585815 3.41424Z" />
        </svg>
      </button>
    </>
  );
}

function Why() {
  const click = useClick();
  return (
    <section className={styles.why} ref={useSectionViewed("Creators Why")}>
      <Title highlight={copy.why.highlight} title={copy.why.title} />
      <div className={styles.whyGrid}>
        {whyCards.map((card) => (
          <a key={card.id} className={`${styles.whyCard} ${styles[`why_${card.id}`] ?? ""}`} href={card.url} target="_blank" rel="noopener noreferrer" onClick={() => click({ place: "Creators Why", title: card.title })}>
            <div className={styles.whyInner}>
              <div className={styles.whyImage}>
                <img src={card.image} alt="" />
              </div>
              <div className={styles.whyText}>
                <h3 className={styles.whyCardTitle}>{card.title}</h3>
                <p className={styles.whyCardDescription}>{card.description}</p>
                <span className={styles.whyCardButton}>{card.buttonLabel}</span>
              </div>
            </div>
          </a>
        ))}
      </div>
    </section>
  );
}

function CreateCard({ card }: { card: CreateCardData }) {
  const click = useClick();
  const [active, setActive] = useState(card.tabs[0]!.title);
  const index = Math.max(0, card.tabs.findIndex((t) => t.title === active));
  const tab = card.tabs[index]!;
  const id = useId();
  const tabbed = card.tabs.length > 1;
  // The keys of the tabs pattern: the arrows go to the next or the previous tab, Home and End to the first and the last.
  const onTabKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    const last = card.tabs.length - 1;
    const steps: Record<string, number> = { ArrowRight: index === last ? 0 : index + 1, ArrowLeft: index === 0 ? last : index - 1, Home: 0, End: last };
    const next = steps[e.key];
    if (next === undefined) return;
    e.preventDefault();
    setActive(card.tabs[next]!.title);
    (e.currentTarget.parentElement?.children[next] as HTMLElement | undefined)?.focus();
  };
  return (
    <div className={styles.card}>
      <div className={styles.cardImage} style={{ "--card-bg": `url("${card.imageBackground}")` } as CSSProperties}>
        <img src={card.image} alt="" />
      </div>
      <div className={styles.cardInfo}>
        <h3 className={styles.cardTitle}>{card.title}</h3>
        <p className={styles.cardDescription}>{card.description}</p>
        <div className={styles.tabContainer}>
          {tabbed && (
            <div className={styles.tabs} role="tablist" aria-label={card.title}>
              {card.tabs.map((t, i) => (
                <button
                  key={t.title}
                  type="button"
                  role="tab"
                  id={`${id}-tab-${i}`}
                  aria-selected={i === index}
                  aria-controls={`${id}-panel`}
                  tabIndex={i === index ? 0 : -1}
                  className={styles.tab}
                  onClick={() => {
                    click({ place: "Creators Create", card: card.id, tab: t.title });
                    setActive(t.title);
                  }}
                  onKeyDown={onTabKey}
                >
                  {t.title}
                </button>
              ))}
            </div>
          )}
          <div className={styles.tabPanel} {...(tabbed ? { role: "tabpanel", id: `${id}-panel`, "aria-labelledby": `${id}-tab-${index}` } : {})}>
          <div className={styles.tabBlock}>
            <p className={styles.tabBlockTitle}>{tab.descriptionTitle}</p>
            <p className={styles.tabBlockText}>{tab.descriptionSubTitle}</p>
          </div>
          <div className={styles.tabBlock}>
            <p className={styles.tabBlockTitle}>{copy.requiredSkills}</p>
            <div className={styles.skills}>
              {tab.skills.map((skill) => (
                <span key={skill} className={styles.skill}>{skill}</span>
              ))}
            </div>
          </div>
          <div className={styles.tabBlock}>
            <p className={styles.tabBlockTitle}>{copy.usefulLinks}</p>
            <div className={styles.links}>
              {tab.links.map((link) => (
                <a
                  key={link.url}
                  className={styles.link + (link.desktopOnly ? ` ${styles.desktopOnly}` : "")}
                  href={link.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={() => click({ place: "Creators Create", card: card.id, tab: tab.title, title: link.label })}
                >
                  {link.label}
                </a>
              ))}
            </div>
          </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function Possibilities() {
  return (
    <section className={styles.create} ref={useSectionViewed("Creators Create")}>
      <Title before={copy.create.before} highlight={copy.create.highlight} after={copy.create.after} />
      <Carousel label={`${copy.create.before}${copy.create.highlight}${copy.create.after}`} items={createCards} renderItem={(card) => <CreateCard card={card} />} keyOf={(card) => card.id} slideWidth={1200} autoplayDelay={0} />
    </section>
  );
}

function SceneCard({ scene }: { scene: HotScene }) {
  const click = useClick();
  const coordinates = `${scene.baseCoords[0]},${scene.baseCoords[1]}`;
  return (
    <Link className={styles.scene} to={`/places/place/${coordinates}`} onClick={() => click({ place: "Creators Live Scenes", title: scene.name })}>
      <div className={styles.sceneImage}>
        {scene.thumbnail ? <img src={scene.thumbnail} alt="" loading="lazy" /> : null}
        <span className={styles.sceneBadge}>{copy.liveScenes.online(scene.usersTotalCount)}</span>
      </div>
      <div className={styles.sceneInfo}>
        <h3 className={styles.sceneTitle}>{scene.name}</h3>
        <p className={styles.sceneCoords}>{coordinates}</p>
      </div>
    </Link>
  );
}

function LiveScenes() {
  const click = useClick();
  const ref = useSectionViewed("Creators Live Scenes");
  const { data } = useQuery(liveScenesQuery());
  const scenes = selectLiveScenes(data ?? []);
  if (scenes.length === 0) return null;
  return (
    <section className={styles.live} ref={ref}>
      <Title highlight={copy.liveScenes.highlight} title={copy.liveScenes.title} />
      <div className={styles.rail}>
        {scenes.map((scene) => (
          <SceneCard key={scene.id} scene={scene} />
        ))}
      </div>
      <ViewAll to="/places" onClick={() => click({ place: "Creators Live Scenes", title: "view-all" })}>{copy.liveScenes.viewAll}</ViewAll>
    </section>
  );
}

function ConnectCard({ card }: { card: ConnectCardData }) {
  const click = useClick();
  const body = (
    <>
      <p className={styles.connectQuote}>&ldquo;{card.description}&rdquo;</p>
      <div className={styles.connectUser}>
        <div className={styles.avatar}>
          <img src={card.image} alt="" />
        </div>
        <p className={styles.connectName}>{card.name}</p>
      </div>
    </>
  );
  return card.url ? (
    <a className={styles.connectCard} href={card.url} target="_blank" rel="noopener noreferrer" draggable={false} onClick={() => click({ place: "Creators Connect", title: card.name })}>
      {body}
    </a>
  ) : (
    <div className={styles.connectCard}>{body}</div>
  );
}

function Connect() {
  const click = useClick();
  return (
    <section className={styles.connect} ref={useSectionViewed("Creators Connect")}>
      <Title highlight={copy.connect.highlight} title={copy.connect.title} />
      <Carousel label={`${copy.connect.highlight} ${copy.connect.title}`} items={connectCards} renderItem={(card) => <ConnectCard card={card} />} keyOf={(card) => card.id} slideWidth={500} autoplayDelay={5000} alignItems="center" />
      <a className={styles.discord} href={copy.discordUrl} target="_blank" rel="noopener noreferrer" onClick={() => click({ place: "Creators Connect", title: "join-discord" })}>
        <span className={styles.discordTitle}>{copy.connect.join}</span>
        <span className={styles.discordIcon}>
          <img src={discordIcon} alt="Discord" />
        </span>
      </a>
    </section>
  );
}

/** A React id made safe for a `url(#…)` reference. */
const svgId = (id: string) => `cl-${id.replace(/[^\w-]/g, "")}`;

function PlayIcon() {
  const filter = svgId(useId());
  return (
    <svg className={styles.play} width="120" height="120" viewBox="0 0 120 120" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <g filter={`url(#${filter})`}>
        <path
          d="M60 16C37.9167 16 20 33.9167 20 56C20 78.0833 37.9167 96 60 96C82.0833 96 100 78.0833 100 56C100 33.9167 82.0833 16 60 16ZM76.9983 59.4183L53.665 73.0833C53.0009 73.334 52.3336 73.5 51.6663 73.5C50.9991 73.5 50.3317 73.4154 49.749 72.9987C48.5837 72.3314 47.8317 70.9154 47.8317 69.5807V42.3349C47.8317 40.8342 48.5804 39.5842 49.749 38.9169C50.9143 38.2496 52.4997 38.2496 53.665 38.9169L76.9983 52.5819C78.1637 53.3306 78.9157 54.4992 78.9157 55.9999C78.9157 57.5005 78.167 58.751 76.9983 59.4183Z"
          fill="white"
        />
      </g>
      <defs>
        <filter id={filter} x="0" y="0" width="120" height="120" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
          <feFlood floodOpacity="0" result="BackgroundImageFix" />
          <feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha" />
          <feOffset dy="4" />
          <feGaussianBlur stdDeviation="10" />
          <feComposite in2="hardAlpha" operator="out" />
          <feColorMatrix type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.55 0" />
          <feBlend mode="normal" in2="BackgroundImageFix" result="effect1_dropShadow" />
          <feBlend mode="normal" in="SourceGraphic" in2="effect1_dropShadow" result="shape" />
        </filter>
      </defs>
    </svg>
  );
}

function Learn() {
  const click = useClick();
  return (
    <section className={styles.learn} ref={useSectionViewed("Creators Learn")}>
      <Title highlight={copy.learn.highlight} title={copy.learn.title} />
      <div className={styles.rail}>
        {learnCards.map((card) => (
          <a key={card.id} className={styles.learnCard} href={card.url} target="_blank" rel="noopener noreferrer" onClick={() => click({ place: "Creators Learn", title: card.title })}>
            <div className={styles.learnVideo}>
              <img src={card.image} alt="" />
              <PlayIcon />
            </div>
            <div className={styles.learnInfo}>
              <div className={styles.learnUserRow}>
                <div className={styles.learnUser}>
                  <div className={styles.learnAvatar}>
                    <img src={card.userImage === "/dcl-logo.svg" ? dclLogo : card.userImage} alt="" />
                  </div>
                  <p className={styles.learnName}>{card.name}</p>
                </div>
                <p className={styles.learnDate}>{card.date}</p>
              </div>
              <h3 className={styles.learnTitle}>{card.title}</h3>
            </div>
          </a>
        ))}
      </div>
      <div className={styles.learnExtra}>
        <div className={`${styles.learnExtraBlock} ${styles.learnExtraFirst}`}>
          {copy.learn.watchMore}
          <a className={`${styles.button} ${styles.buttonPrimary}`} href={copy.learn.watchMoreUrl} target="_blank" rel="noopener noreferrer" onClick={() => click({ place: "Creators Learn", title: "watch-more" })}>
            {copy.learn.watchMoreButton}
          </a>
        </div>
        <div className={`${styles.learnExtraBlock} ${styles.learnExtraSecond}`}>
          {copy.learn.yourTutorial}
          <a className={`${styles.button} ${styles.buttonLight}`} href={copy.learn.yourTutorialUrl} target="_blank" rel="noopener noreferrer" onClick={() => click({ place: "Creators Learn", title: "submit-tutorial" })}>
            {copy.learn.yourTutorialButton}
          </a>
        </div>
      </div>
    </section>
  );
}

/** The reference's blog card date: "October 5, 2026", read in UTC on both renders. */
function longDate(value: string): string {
  const parsed = new Date(value.includes("T") ? value : `${value} UTC`);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric", timeZone: "UTC" });
}

function BlogCard({ post }: { post: BlogPostSummary }) {
  const click = useClick();
  return (
    <Link className={styles.blogCard} to={post.url} onClick={() => click({ place: "Creators Blog", title: post.title })}>
      <div className={styles.blogImage}>{post.image ? <img src={post.image.url} alt="" loading="lazy" /> : null}</div>
      <div className={styles.blogInfo}>
        <div className={styles.blogMeta}>
          {post.category.title ? <p className={styles.blogCategory}>{post.category.title}</p> : null}
          <p className={styles.blogDate}>{longDate(post.publishedDate)}</p>
        </div>
        <h3 className={styles.blogTitle}>{post.title}</h3>
      </div>
    </Link>
  );
}

/** The newest posts of the blog, as the reference reads them from the CMS. */
function Blog() {
  const click = useClick();
  const ref = useSectionViewed("Creators Blog");
  const { data } = useQuery(latestPostsQuery());
  const posts = data ?? [];
  if (posts.length === 0) return null;
  return (
    <section className={styles.blog} ref={ref}>
      <Title highlight={copy.blog.highlight} title={copy.blog.title} />
      <div className={styles.blogCards}>
        {posts.map((post) => (
          <BlogCard key={post.id} post={post} />
        ))}
      </div>
      <ViewAll to="/blog" onClick={() => click({ place: "Creators Blog", title: "view-all" })}>{copy.blog.viewAll}</ViewAll>
    </section>
  );
}

function CircleAndArrow({ open }: { open: boolean }) {
  const clip = svgId(useId());
  return (
    <svg className={styles.faqIcon + (open ? ` ${styles.faqIconOpen}` : "")} width="72" height="72" viewBox="0 0 72 72" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <circle opacity={open ? "1" : "0.2"} cx="36" cy="36" r="35" stroke="white" fill={open ? "white" : "none"} strokeWidth="2" />
      <g clipPath={`url(#${clip})`}>
        <path d="M45 33.0022L42.885 30.8872L36 37.7572L29.115 30.8872L27 33.0022L36 42.0022L45 33.0022Z" fill={open ? "#242129" : "white"} />
      </g>
      <defs>
        <clipPath id={clip}>
          <rect width="36" height="36" fill="white" transform="translate(54 18) rotate(90)" />
        </clipPath>
      </defs>
    </svg>
  );
}

function Faq({ question, children, index, open, onToggle }: { question: string; children: ReactNode; index: number; open: boolean; onToggle: () => void }) {
  const questionId = `create-faq-question-${index}`;
  const answerId = `create-faq-answer-${index}`;
  return (
    <div className={styles.faq}>
      <button type="button" className={styles.faqRow} aria-expanded={open} aria-controls={answerId} onClick={onToggle}>
        <span className={styles.faqQuestion} id={questionId}>{question}</span>
        <CircleAndArrow open={open} />
      </button>
      <div className={styles.faqAnswerWrap + (open ? ` ${styles.faqOpen}` : "")} id={answerId} role="region" aria-labelledby={questionId} aria-hidden={!open}>
        <p className={styles.faqAnswer}>{children}</p>
      </div>
    </div>
  );
}

function Faqs() {
  const click = useClick();
  const [expanded, setExpanded] = useState<number | null>(null);
  // Only opening a question was a Click in the reference; closing one sent nothing.
  const toggle = (index: number, question: string) => {
    if (expanded !== index) click({ place: "Creators Faqs", title: question });
    setExpanded(expanded === index ? null : index);
  };
  return (
    <section className={styles.faqs} aria-label="Frequently Asked Questions" ref={useSectionViewed("Creators Faqs")}>
      <div className={styles.faqsBorder}>
        <div className={styles.faqsContainer}>
          <p className={styles.faqsSubtitle}>{copy.faqs.subtitle}</p>
          <h2 className={`${styles.sectionTitle} ${styles.faqsTitle}`}>{copy.faqs.title}</h2>
          {faqItems.map((faq, index) => (
            <Faq key={faq.question} question={faq.question} index={index} open={expanded === index} onToggle={() => toggle(index, faq.question)}>
              {faq.answer}
            </Faq>
          ))}
          <a className={styles.faqsCta} href={copy.faqs.ctaUrl} onClick={() => click({ place: "Creators Faqs", title: "faqs-cta" })}>
            {copy.faqs.cta}
          </a>
        </div>
      </div>
    </section>
  );
}
