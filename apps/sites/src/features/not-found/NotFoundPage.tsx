// The fullscreen page an unknown path lands on, outside the
// site's chrome (no nav, no footer). The root error boundary renders it for a 404 (routes/root.tsx), so the response
// keeps its status; the CTA is a plain link to /whats-on, the What's On page.
import { Link } from "react-router";

import { AnimatedBackground, Button, GeneralDecentralandColor } from "@dcl/one-ui";

import notFoundAvatar from "@dcl/one-ui/apps/sites/assets/errors/not-found-avatar.webp";
import styles from "./NotFoundPage.module.css";

export default function NotFoundPage() {
  return (
    <main className={styles.page} data-status="404">
      <div className={styles.background}>
        <AnimatedBackground />
      </div>
      <Link className={styles.home} to="/" aria-label="Decentraland Home">
        <GeneralDecentralandColor className={styles.logo} aria-hidden="true" />
      </Link>
      <div className={styles.content}>
        <div className={styles.text}>
          <p className={styles.watermark} aria-hidden="true">404</p>
          <h1 className={styles.title}>
            Oops! <span className={styles.titleRest}>This page has left the world.</span>
          </h1>
          <p className={styles.description}>
            The link may be outdated, but Decentraland is still live. Find community events, DJ sets, creator-built games, and
            new experiences happening in-world.
          </p>
        </div>
        <Button size="lg" className={styles.cta} href="/whats-on">
          See Events
        </Button>
      </div>
      <div className={styles.graphic}>
        <img className={styles.avatar} src={notFoundAvatar} width={801} height={698} alt="" />
      </div>
    </main>
  );
}
