// The card a submission lands on. Only the navigation the
// form makes carries `{ submitted: true }`; any other visit goes to /report. History state exists only in the
// browser, so the server and the first client render show the background and the check runs after hydration.
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import { AnimatedBackground } from "@dcl/one-ui";
import SitesLayout from "../../components/chrome/SitesLayout";

import { GeneralDecentralandColor } from "@dcl/one-ui/foundations/icons";
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
    <SitesLayout active="legal" overlayNav>
      <main className={styles.page}>
        <AnimatedBackground />
        {shown && (
          <div className={styles.content}>
            <div className={styles.card}>
              <div className={styles.logoWrap}>
                <GeneralDecentralandColor className={styles.logo} aria-hidden="true" />
                <h1 className={styles.title}>Report Submitted</h1>
              </div>
              <div className={styles.text}>
                <p className={styles.body}>Your report has been received and will be reviewed shortly.</p>
                <p className={styles.secondary}>You can close this window now.</p>
              </div>
            </div>
          </div>
        )}
      </main>
    </SitesLayout>
  );
}
