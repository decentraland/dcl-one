// from: sites@17b3535a:src/components/Report/ReportSuccess: the card a submission lands on. Only the navigation the
// form makes carries `{ submitted: true }`; any other visit goes to /report. History state exists only in the
// browser, so the server and the first client render show the background and the check runs after hydration.
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";
import SitesChrome from "@ui/web/frames/SitesChrome";

import DclLogo from "../../components/DclLogo";
import styles from "./ReportSuccess.module.css";

export default function ReportSuccess() {
  const location = useLocation();
  const navigate = useNavigate();
  const [shown, setShown] = useState(false);
  const submitted = (location.state as { submitted?: boolean } | null)?.submitted === true;

  useEffect(() => {
    if (submitted) setShown(true);
    else navigate("/report", { replace: true });
  }, [submitted, navigate]);

  return (
    <SitesChrome active="legal" overlayNav>
      <main className={styles.page}>
        <AnimatedBackground />
        {shown && (
          <div className={styles.content}>
            <div className={styles.card}>
              <div className={styles.logoWrap}>
                <DclLogo className={styles.logo} />
                <h3 className={styles.title}>Report Submitted</h3>
              </div>
              <div className={styles.text}>
                <p className={styles.body}>Your report has been received and will be reviewed shortly.</p>
                <p className={styles.secondary}>You can close this window now.</p>
              </div>
            </div>
          </div>
        )}
      </main>
    </SitesChrome>
  );
}
