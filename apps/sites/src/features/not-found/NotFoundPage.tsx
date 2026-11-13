// from: sites@17b3535a:src/pages/not-found/NotFoundPage.tsx: the fullscreen page an unknown path lands on, outside the
// site's chrome (no nav, no footer). The root error boundary renders it for a 404 (routes/root.tsx), so the response
// keeps its status; the CTA opens What's On, the reference's /events, as a document load.
import { Link } from "react-router";

import { Button } from "@dcl/one-ui/components";
import { asset } from "@ui/asset";
import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";

import DclLogo from "../components/DclLogo";
import styles from "./NotFoundPage.module.css";

export default function NotFoundPage() {
  return (
    <main className={styles.page} data-status="404">
      <div className={styles.background}>
        <AnimatedBackground />
      </div>
      <Link className={styles.home} to="/" aria-label="Decentraland Home">
        <DclLogo className={styles.logo} />
      </Link>
      <div className={styles.content}>
        <div className={styles.text}>
          <p className={styles.watermark} aria-hidden="true">404</p>
          <h1 className={styles.title}>
            {"Oops!"} <span className={styles.titleRest}>This page has left the world.</span>
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
        <img className={styles.avatar} src={asset("assets/errors/not-found-avatar.webp")} width={801} height={698} alt="" />
      </div>
    </main>
  );
}
