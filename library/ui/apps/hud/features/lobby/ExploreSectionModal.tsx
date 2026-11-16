// One section at a time: a second open while one is up is ignored, and the full page opening takes
// over from the modal.

import { useEffect } from 'react'
import { Button, MaskIcon, openPopup } from '../../design'
import closeIcon from '../../assets/events/close-icn-thin.png'
import { EventsPage } from '../events/EventsPage'
import { MapPage } from '../map/MapPage'
import { PlacesPage } from '../places/PlacesPage'
import { useSession } from '../session/SessionContext'
import styles from './ExploreSectionModal.module.css'

export type ExploreSection = 'map' | 'places' | 'events'

const LABEL: Record<ExploreSection, string> = { map: 'Map', places: 'Places', events: 'Events' }

let shown: ExploreSection | null = null

const ignore = (): void => undefined

function ExploreSectionModal({ section, onClose }: { section: ExploreSection; onClose: () => void }): React.JSX.Element {
  const session = useSession()
  const compact = { onTravel: onClose }
  const pageOpen = session.map.open || session.places.open || session.events.open
  useEffect(() => {
    if (pageOpen) onClose()
  }, [pageOpen, onClose])
  useEffect(
    () => () => {
      shown = null
    },
    []
  )
  return (
    <div className={styles.panel} role="dialog" aria-modal="true" aria-label={LABEL[section]} onClick={(e) => e.stopPropagation()}>
      <div className={section === 'places' ? `${styles.host} ${styles.places}` : styles.host}>
        {section === 'map' && <MapPage map={session.map} minimap={session.minimap} profile={session.profile} gallery={session.gallery} compact={compact} onNavigate={ignore} />}
        {section === 'places' && (
          <PlacesPage
            places={session.places}
            account={session.profile.data?.address}
            profile={session.profile}
            compact={compact}
            onNavigate={ignore}
            onTeleport={(x, y) => session.map.teleportToPlace(x, y)}
            onVisitWorld={(realm) => session.map.changeRealm(realm)}
          />
        )}
        {section === 'events' && <EventsPage events={session.events} profile={session.profile} friends={session.friends} compact={compact} onNavigate={ignore} />}
      </div>
      <Button size="bare" radius="md" variant="shade" className={styles.close} aria-label={`Close ${LABEL[section]}`} onClick={onClose}>
        <MaskIcon src={closeIcon} size={14} />
      </Button>
    </div>
  )
}

export function openExploreSection(section: ExploreSection): void {
  if (shown != null) return
  shown = section
  openPopup((close) => <ExploreSectionModal section={section} onClose={close} />, {
    className: styles.scrim,
    onClose: () => {
      shown = null
    }
  })
}
