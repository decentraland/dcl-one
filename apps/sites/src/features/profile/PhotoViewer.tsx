import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Link } from "react-router";

import { Button, JumpInCTA } from "@dcl/one-ui";
import { ProfileAvatar } from "@dcl/one-ui/apps/sites/provisional/ProfileAvatar/ProfileAvatar";
import {
  GeneralClose,
  GeneralInfoButton,
  GeneralLeftArrow,
  GeneralMapPin,
  GeneralRightArrow,
  MenusDownload,
} from "@dcl/one-ui/foundations/icons";

import styles from "./PhotoViewer.module.css";

export type PhotoViewerPerson = {
  name: string;
  /** Their profile page; without one the name is plain text. */
  href?: string;
  faceUrl?: string;
  isGuest: boolean;
};

export type PhotoViewerDetails = {
  /** Already formatted; left out when the service's date cannot be read. */
  date?: string;
  takenBy?: { name: string; href: string };
  place: { label: string; jumpInHref?: string };
  people: PhotoViewerPerson[];
};

export type PhotoViewerProps = {
  imageUrl: string;
  /** The image's alt text. */
  alt: string;
  /** The dialog's accessible name, e.g. "Photo at Genesis Plaza, November 08, 2026, 3 of 24". */
  label: string;
  details: PhotoViewerDetails;
  /**
   * Where Download saves the full-size image from, and its file name. The node serves reel images
   * from the page's origin; a browser opens a cross-origin image instead of saving it.
   */
  download?: { href: string; fileName: string };
  /** Without one, its button is marked unavailable and its arrow key does nothing. */
  onPrevious?: () => void;
  onNext?: () => void;
  onClose: () => void;
};

const FOCUSABLE = 'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * The profile photo viewer: a full-screen dialog with one photo and its information panel, drawn
 * here because the design system has no dialog yet. It traps focus, closes on Escape or a click on
 * the backdrop, locks the page's scroll, and moves between photos with the arrow keys and the
 * Previous and Next buttons; focus returns to where it was when it closes. It leaves out three parts
 * of the source's PhotoModal: Share to X and Copy link, which share a `/reels/:id` page dcl-one does
 * not serve, and each person's wearables list, which needs a subgraph read.
 */
export function PhotoViewer(props: PhotoViewerProps) {
  // The dialog is portalled to <body>, which only exists in the browser.
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  return mounted ? createPortal(<ViewerDialog {...props} />, document.body) : null;
}

function ViewerDialog({ imageUrl, alt, label, details, download, onPrevious, onNext, onClose }: PhotoViewerProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const [detailsVisible, setDetailsVisible] = useState(true);
  const titleId = useId();

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeRef.current?.focus();
    const { overflow } = document.body.style;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = overflow;
      if (opener?.isConnected) opener.focus();
    };
  }, []);

  // Stepping to another photo, or hiding the panel, can unmount the link that held focus; the
  // dialog takes it back so its keys keep working. The first render is left to the effect above,
  // which records where focus came from.
  const rendered = useRef(false);
  useLayoutEffect(() => {
    if (!rendered.current) {
      rendered.current = true;
      return;
    }
    const dialog = dialogRef.current;
    if (dialog && !dialog.contains(document.activeElement)) dialog.focus();
  }, [imageUrl, detailsVisible]);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
    } else if (event.key === "ArrowLeft" && onPrevious) {
      event.preventDefault();
      onPrevious();
    } else if (event.key === "ArrowRight" && onNext) {
      event.preventDefault();
      onNext();
    } else if (event.key === "Tab") {
      const focusable = [...(dialogRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])];
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) return;
      const active = document.activeElement;
      // The dialog itself (focused after a click on the photo) counts as outside its controls.
      const outside = active === dialogRef.current || !dialogRef.current?.contains(active);
      if (event.shiftKey && (active === first || outside)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || outside)) {
        event.preventDefault();
        first.focus();
      }
    }
  }

  return (
    <div className={styles.backdrop} onClick={onClose}>
      <div
        ref={dialogRef}
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        // A click inside on something that cannot take focus keeps focus in the dialog, so its keys still work.
        tabIndex={-1}
        aria-label={label}
        onKeyDown={onKeyDown}
        onClick={(event) => event.stopPropagation()}
      >
        <span className={`${styles.control} ${styles.close}`}>
          <Button ref={closeRef} iconOnly size="sm" aria-label="Close" onClick={onClose}>
            <GeneralClose size={24} aria-hidden="true" />
          </Button>
        </span>
        <div className={detailsVisible ? `${styles.body} ${styles.withDetails}` : styles.body}>
          <div className={styles.media}>
            <div className={styles.toolbar}>
              {download ? (
                <span className={styles.control}>
                  <Button iconOnly size="sm" href={download.href} download={download.fileName} aria-label="Download photo">
                    <MenusDownload size={22} aria-hidden="true" />
                  </Button>
                </span>
              ) : null}
              <span className={detailsVisible ? `${styles.control} ${styles.controlOn}` : styles.control}>
                <Button
                  iconOnly
                  size="sm"
                  aria-label="Information"
                  aria-pressed={detailsVisible}
                  onClick={() => setDetailsVisible((visible) => !visible)}
                >
                  <GeneralInfoButton size={22} aria-hidden="true" />
                </Button>
              </span>
            </div>
            <img className={styles.image} src={imageUrl} alt={alt} />
            {onPrevious || onNext ? (
              <>
                <StepButton
                  label="Previous photo"
                  className={styles.previous}
                  onStep={onPrevious}
                  icon={<GeneralLeftArrow size={24} aria-hidden="true" />}
                />
                <StepButton
                  label="Next photo"
                  className={styles.next}
                  onStep={onNext}
                  icon={<GeneralRightArrow size={24} aria-hidden="true" />}
                />
              </>
            ) : null}
          </div>
          {detailsVisible ? (
            <section className={styles.details} aria-labelledby={titleId}>
              <div className={styles.header}>
                <h2 id={titleId} className={styles.sectionTitle}>
                  information
                </h2>
                {details.date ? <p className={styles.date}>{details.date}</p> : null}
                {details.takenBy ? (
                  <p className={styles.takenBy}>
                    Photo taken by{" "}
                    <Link to={details.takenBy.href} prefetch="intent" className={styles.link}>
                      {details.takenBy.name}
                    </Link>
                  </p>
                ) : null}
              </div>
              <h3 className={`${styles.sectionTitle} ${styles.padded}`}>place</h3>
              <div className={styles.place}>
                <span className={styles.placeLabel}>
                  <GeneralMapPin size={20} aria-hidden="true" />
                  <span>{details.place.label}</span>
                </span>
                {details.place.jumpInHref ? (
                  <JumpInCTA href={details.place.jumpInHref} size="sm" aria-label={`Jump in to ${details.place.label}`} />
                ) : null}
              </div>
              {details.people.length > 0 ? (
                <>
                  <h3 className={`${styles.sectionTitle} ${styles.padded}`}>people</h3>
                  <ul className={styles.people}>
                    {details.people.map((person, index) => (
                      <li key={`${person.href ?? person.name}-${index}`} className={styles.person}>
                        <span aria-hidden="true">
                          <ProfileAvatar
                            imageUrl={person.faceUrl}
                            name={person.name}
                            color="var(--dcl-neutrals-pale-black)"
                            size={40}
                          />
                        </span>
                        {person.href ? (
                          <Link to={person.href} prefetch="intent" className={styles.personName}>
                            {person.name}
                          </Link>
                        ) : (
                          <span className={styles.personName}>{person.name}</span>
                        )}
                        {person.isGuest ? <span className={styles.guest}>guest</span> : null}
                      </li>
                    ))}
                  </ul>
                </>
              ) : null}
            </section>
          ) : null}
        </div>
      </div>
    </div>
  );
}

/**
 * Previous or Next. At either end it stays mounted and is only marked unavailable (Button keeps an
 * `aria-disabled` button focusable and ignores its clicks): removing or disabling the button that
 * holds focus would drop focus out of the dialog.
 */
function StepButton({
  label,
  className,
  onStep,
  icon,
}: {
  label: string;
  className: string | undefined;
  onStep?: () => void;
  icon: ReactNode;
}) {
  return (
    <span className={`${styles.control} ${styles.step} ${className ?? ""}`}>
      <Button iconOnly size="sm" aria-label={label} aria-disabled={!onStep} onClick={onStep}>
        {icon}
      </Button>
    </span>
  );
}
