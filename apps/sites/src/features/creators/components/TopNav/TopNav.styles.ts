import styled from '@emotion/styled'
import { media } from '@/data/lib/creators/theme'

// Holds the space of the lazy-loaded global DCL navbar (same height) so there's no layout shift; the
// fill matches the restyled navbar bar (see NavbarViolet below) so it doesn't flash when it hydrates.
export const Skeleton = styled.div`
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: var(--nav-h);
  /* Solid stand-in for the bar's #161518 at 40% over the purple field. */
  background: #36184a;
  z-index: 50;
`

// Restyles the shared decentraland-ui2 Navbar to the violet Figma design. ui2 hardcodes the navbar's
// colors in its own Emotion styled-components with no theme hook, and the DCL preference is to
// override in the consumer rather than fork ui2. Selectors target the navbar's stable rendering
// contract — semantic tags, the logo / hamburger aria-labels, and the `.active` tab class — since
// ui2's Emotion class names are hashed and not stable. `display: contents` keeps this wrapper from
// generating a box (the navbar itself is position: fixed). Specificity (wrapper class +
// element/attribute) beats ui2's single-class rules, so no !important is needed.
//
// ⚠️ These structural selectors (`nav > div:first-of-type > …`) depend on decentraland-ui2's internal
// DOM nesting, and the hamburger selectors on its English `aria-label`s ("Open menu" / "Close menu").
// Validated against decentraland-ui2@3.20.0 — re-check on upgrade (a wrapper div added/removed, or a
// localized aria-label, would silently drop these overrides).
export const NavbarViolet = styled.div`
  display: contents;

  /* Bar background: translucent near-black (#161518 at 40%, per the designer) over the page field. It
     deepens to 80% once the page scrolls (body[data-scrolled], set by NavBar) so it doesn't wash out
     over light content passing underneath. ui2's own drop shadow is kept, as on sites' navbar. */
  & nav::before {
    background: rgba(22, 21, 24, 0.4);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    transition: background 0.25s ease;
  }
  body[data-scrolled] & nav::before {
    background: rgba(22, 21, 24, 0.8);
  }

  /* Desktop nav tabs (Explore / Shop / Create / Learn): light text on the dark bar. Direct-child
     selectors deliberately exclude the dark dropdown panels that open on hover. */
  & nav > div:first-of-type > div > a,
  & nav > div:first-of-type > div > div > button {
    color: #ecebed;
  }
  & nav > div:first-of-type > div > a:hover,
  & nav > div:first-of-type > div > div > button:hover {
    color: var(--color-white);
    background-color: rgba(255, 255, 255, 0.12);
  }
  & nav > div:first-of-type > div > a.active,
  & nav > div:first-of-type > div > div > button.active {
    color: var(--color-white);
    background-color: rgba(255, 255, 255, 0.18);
  }

  /* ui2 sizes the avatar only under max-width: 991px / min-width: 992px. At a fractional viewport
     width in between (browser zoom, DPR scaling) neither matches, the button has no size and the
     <img> at width: 100% renders at its intrinsic size. Clamp to ui2's desktop size as a guard. */
  & nav button[aria-label='User menu'] {
    max-width: 48px;
    max-height: 48px;
  }

  /* Signed-in right cluster: force the cluster's buttons and any svg glyph to white so they read on
     the dark bar. The profile pic is an <img>, so it isn't affected by color/fill. */
  & nav > div:last-of-type button {
    color: var(--color-soft-white);
  }
  & nav > div:last-of-type svg {
    color: var(--color-soft-white);
  }
  /* Only paths that are FILLED shapes — glyphs that draw strokes over fill="none" must keep them. The
     profile panel (the avatar button's sibling card on desktop, ui2's data-mobile-user-card on mobile)
     is left alone: its chain logos are multi-colour brand marks that must keep their own fills. */
  &
    nav
    > div:last-of-type
    svg:not(button[aria-label='User menu'] ~ div svg, [data-mobile-user-card] svg)
    path:not([stroke]) {
    fill: currentColor;
  }

  /* Sign-in button (signed-out state): light outline + text on the dark bar. The hamburger is
     excluded via :not([aria-label]). */
  & nav > div:last-of-type > button:not([aria-label]) {
    color: var(--color-soft-white);
    border-color: var(--color-soft-white);
  }
  & nav > div:last-of-type > button:not([aria-label]):hover {
    background-color: rgba(255, 255, 255, 0.12);
    border-color: var(--color-soft-white);
  }
  & nav > div:last-of-type > button:not([aria-label]):active {
    background-color: rgba(255, 255, 255, 0.18);
    border-color: var(--color-soft-white);
  }

  /* Mobile hamburger / menu button: solid purple with a white icon — ui2's default is a faint
     white-on-dark chip. The icon uses currentColor, so the color prop drives it. */
  & nav button[aria-label='Open menu'],
  & nav button[aria-label='Close menu'] {
    background-color: var(--color-accent);
    color: var(--color-white);
  }
  & nav button[aria-label='Open menu']:hover,
  & nav button[aria-label='Close menu']:hover {
    background-color: var(--color-accent-hover);
  }
  & nav button[aria-label='Open menu']:active,
  & nav button[aria-label='Close menu']:active {
    background-color: var(--color-accent-active);
  }

  /* Mobile bar. ui2 already gives it 64px at 12/16 padding, so only the fill, the spacing of the
     right cluster and the menu chip differ. */
  ${media.maxWidth('mobile')} {
    & nav::before {
      background: rgba(22, 21, 24, 0.75);
    }

    /* 24px between avatar and menu. ui2 splits that cluster in two nested flex rows (16 outer, 12
       inner at this width), so both have to carry it or the items end up unevenly spaced. */
    & nav > div:last-of-type,
    & nav > div:last-of-type > div {
      gap: 24px;
    }

    /* A translucent white chip, NOT the purple fill above: the mobile design draws it as glass on
       the bar. */
    & nav button[aria-label='Open menu'],
    & nav button[aria-label='Close menu'] {
      width: 40px;
      height: 40px;
      border-radius: 8px;
      background-color: rgba(255, 255, 255, 0.1);
      color: var(--color-white);
    }
    & nav button[aria-label='Open menu']:hover,
    & nav button[aria-label='Close menu']:hover,
    & nav button[aria-label='Open menu']:active,
    & nav button[aria-label='Close menu']:active {
      background-color: rgba(255, 255, 255, 0.18);
    }
    & nav button[aria-label='Open menu'] svg,
    & nav button[aria-label='Close menu'] svg {
      width: 32px;
      height: 32px;
    }

    /* The avatar carries a half-white ring at this size. */
    & nav button[aria-label='User menu'] img {
      border: 2px solid rgba(255, 255, 255, 0.5);
      border-radius: 100px;
    }
  }
`
