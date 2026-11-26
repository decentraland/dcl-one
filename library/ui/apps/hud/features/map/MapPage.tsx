// The full-screen map: Genesis City with its marker layers, the category chips, the controls
// and Layers panel, and the panel of search results, places and events on the right.

import { MaskIcon } from '@dcl/one-ui'
import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { DEFAULT_REALM, serviceUrl } from '@hud-host'
import { isGenesisSatelliteView } from '@explorer-core/features/map/atlas'
import { homeOf, isPlacePanelHome, placeHomeTarget } from '@explorer-core/features/map/home'
import { isRoad, isWorldPlace, type PlacePhoto } from '@explorer-core/features/map/mapApi'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import type { EventData } from '@explorer-core/engine/protocol'
import { parseParcel, playerMarkerPosition, worldToGrid, type Vec2 } from '@explorer-core/features/map/mapCoords'
import { mapLayers } from '@explorer-core/features/map/mapLayersStore'
import type { EventMarker, PlaceMarker } from '@explorer-core/features/map/mapMarkers'
import { CAMERA_CENTER_SECONDS, CAMERA_MOVE_SECONDS } from '@explorer-core/features/map/mapZoom'
import { isRouteTo, navigation } from '@explorer-core/features/map/navigation'
import { Navmap } from '@explorer-core/features/map/navmap'
import timeIcon from '../../assets/events/time-icn.png'
import { showToast } from '../../design'
import { usePhotoViewer } from '../gallery/usePhotoViewer'
import { travelToPhoto } from '../gallery/reelActions'
import { openPassport } from '../profile/Passport'
import { MainMenuShell } from '../menu/MainMenuShell'
import { useOptionalSession } from '../session/SessionContext'
import type { GalleryState, MapState, MinimapState, ProfileState, TravelOutcome } from '../session/useEngineSession'
import { CategoryStrip } from './CategoryStrip'
import { DestinationBanner } from './DestinationBanner'
import { LayersPanel } from './LayersPanel'
import { MapCanvas } from './MapCanvas'
import { MapToolbar } from './MapToolbar'
import { RouteLayer } from './RouteLayer'
import { WorldWarning } from './WorldWarning'
import { EventInfo } from './panel/EventInfo'
import { PlaceInfo } from './panel/PlaceInfo'
import { RightPanel } from './panel/RightPanel'
import { SearchBar } from './panel/SearchBar'
import { SearchResults } from './panel/SearchResults'
import { useMapCamera } from './useMapCamera'
import { useMapLayerData } from './useMapLayerData'
import { useMapGridAge } from './useMapGridAge'
import noticeStyles from './MapNotices.module.css'
import styles from './MapPage.module.css'

// Dismissing the worlds warning holds for the session.
let worldWarningClosed = false

const ARRIVAL_CHECK_MS = 250

export function MapPage({
  map,
  minimap,
  profile,
  gallery,
  compact,
  onNavigate
}: {
  map: MapState
  minimap: MinimapState
  profile: ProfileState
  gallery: GalleryState
  /** Shown in the lobby's section modal; `onTravel` closes it once a jump goes ahead. */
  compact?: { onTravel: () => void }
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  const open = map.open || compact != null
  const camera = useMapCamera()
  const fetchSatelliteTile = useOptionalSession()?.fetchSatelliteTile
  const layers = useSyncExternalStore(mapLayers.subscribe, mapLayers.getSnapshot)
  const nav = useMemo(() => new Navmap(), [])
  const snap = useSyncExternalStore(nav.subscribe, nav.getSnapshot)
  const home = useMemo(() => homeOf(map.home), [map.home])
  const destination = useSyncExternalStore(navigation.subscribe, navigation.getSnapshot)
  const [layersOpen, setLayersOpen] = useState(false)
  const [selected, setSelected] = useState<string | null>(null)
  const [warning, setWarning] = useState(false)
  const [photo, setPhoto] = useState<{ photos: PlacePhoto[]; index: number } | null>(null)

  const genesis = isGenesisSatelliteView(minimap.satelliteView)
  const address = profile.data?.address
  const { pose } = minimap
  const data = useMapLayerData(open, layers, address)
  // The parcels map is drawn from the map API's grid; the satellite map is not.
  const gridAge = useMapGridAge(serviceUrl('mapApi'), open && layers.has('parcels'))

  const { translateTo, setPosition, resetZoom } = camera
  useEffect(() => {
    nav.setEffects({ moveCamera: translateTo })
  }, [nav, translateTo])

  useEffect(() => {
    if (open) {
      resetZoom()
      setPosition(worldToGrid(pose.current.x, pose.current.z))
      nav.activate()
      setWarning(!genesis && !worldWarningClosed)
    } else {
      nav.deactivate()
      setSelected(null)
      setPhoto(null)
      setWarning(false)
    }
    // the map opening or closing, nothing else
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  // the route is followed whether or not the map is open, and only in Genesis City: a World's
  // parcels share the numbers but not the place
  useEffect(() => {
    if (destination == null || !genesis) return
    const timer = setInterval(() => {
      if (navigation.playerMoved(playerMarkerPosition(pose.current.x, pose.current.z))) showToast('You have arrived to your destination')
    }, ARRIVAL_CHECK_MS)
    return () => clearInterval(timer)
  }, [destination, pose, genesis])

  // A refused pick leaves the page as it is; the startup lobby's pick closes the page itself, so a
  // toggle after it would reopen the map.
  const go = (travel: () => TravelOutcome, travelled: () => void): void => {
    const outcome = travel()
    if (outcome === 'refused') return
    travelled()
    if (compact != null) compact.onTravel()
    else if (outcome === 'sent') map.toggle()
  }

  const onParcel = useCallback(
    (parcel: Vec2): void => {
      setSelected(null)
      translateTo(parcel, CAMERA_MOVE_SECONDS)
      void nav.selectParcel(parcel, true)
    },
    [nav, translateTo]
  )
  const onEvent = useCallback(
    (marker: EventMarker): void => {
      setSelected(`liveEvents:${marker.id}`)
      void nav.selectEvent(marker.event)
    },
    [nav]
  )
  const onPlace = useCallback(
    (layer: 'pois' | 'search' | 'category', marker: PlaceMarker): void => {
      setSelected(`${layer}:${marker.id}`)
      // the place opens at once, then again as the parcel it stands on once that has been read
      void nav.selectPlace(marker.place, { fromSearch: true })
      const base = parseParcel(marker.place.base_position)
      if (base != null) void nav.selectParcel(base, true)
    },
    [nav]
  )
  const showHome = useCallback((): void => {
    if (home.world != null) return
    const parcel = home.parcel ?? { x: 0, y: 0 }
    setSelected(null)
    void nav.selectParcelAsHome(parcel)
  }, [home, nav])

  const openPhoto = open && photo != null ? photo.photos[photo.index] : undefined
  usePhotoViewer(
    photo != null && openPhoto != null
      ? {
          photos: photo.photos.map((ph) => ({ id: ph.id, url: ph.url, thumbnailUrl: ph.thumbnailUrl, dateTime: ph.dateTime ?? '' })),
          index: photo.index,
          meta: gallery.metas[openPhoto.id],
          canEdit: false,
          onIndex: (index) => setPhoto({ photos: photo.photos, index }),
          onLoadMeta: gallery.loadPhoto,
          loadWearables: gallery.loadWearables,
          onClose: () => setPhoto(null),
          onViewPerson: (u) => openPassport(u.address),
          onJumpIn: (place) =>
            go(
              () => travelToPhoto(place, map),
              () => {
                setPhoto(null)
                nav.clearInput()
              }
            )
        }
      : null
  )

  if (!open) return null

  const jumpInPlace = (): void => {
    const view = snap.place
    if (view == null) return
    const { place, origin, landOnOrigin } = view
    const base = parseParcel(place.base_position)
    const target = (landOnOrigin || isRoad(place.title)) && origin != null ? origin : base
    if (isWorldPlace(place)) {
      if (place.world_name == null) return
      const realm = place.world_name
      go(() => map.changeRealm(realm), () => nav.clearInput())
    } else {
      if (target == null) return
      const routeEnds = isRouteTo(destination, place)
      go(
        () => map.teleportToPlace(target.x, target.y),
        () => {
          if (routeEnds) navigation.stop()
          nav.clearInput()
        }
      )
    }
  }
  const jumpInEvent = (): void => {
    const view = snap.event
    if (view == null) return
    go(() => map.teleportToPlace(view.event.x, view.event.y), () => nav.clearInput())
  }
  const placeIsHome = (place: DiscoverPlace, origin: Vec2 | null): boolean => isPlacePanelHome(home, place, origin)
  const toggleHome = (place: DiscoverPlace, origin: Vec2 | null): void => {
    if (placeIsHome(place, origin)) return map.clearHome()
    const target = placeHomeTarget(place, origin)
    if (target != null) map.setHome(target.realm ?? DEFAULT_REALM, target.parcel)
  }

  const p = profile.data
  const body = (
    <>
      <div className={compact != null ? `${styles.wrap} ${styles.compact}` : styles.wrap}>
        <MapCanvas
          camera={camera}
          open={open}
          layers={layers}
          genesis={genesis}
          pose={pose}
          liveEvents={data.liveEvents}
          pois={data.pois}
          people={data.people}
          pins={data.pins}
          searchMarkers={snap.searchMarkers}
          categoryMarkers={snap.categoryMarkers}
          markerCategory={snap.markerCategory}
          categoryLayer={snap.categoryLayer}
          home={home.world == null ? home.parcel : null}
          selected={selected}
          resultHover={snap.resultHover}
          resultSelected={snap.resultSelected}
          onParcel={onParcel}
          onEvent={onEvent}
          onPlace={onPlace}
          onHome={showHome}
          fetchSatelliteTile={fetchSatelliteTile}
          satelliteDetail={open && genesis}
        >
          <RouteLayer destination={destination} pose={pose} active={open} />
        </MapCanvas>

        <CategoryStrip selected={snap.chip} wrap={compact != null} onSelect={(key) => nav.selectCategory(key)} />
        {warning && (
          <WorldWarning
            onClose={() => {
              worldWarningClosed = true
              setWarning(false)
            }}
          />
        )}
        {destination != null && <DestinationBanner destination={destination} onExit={navigation.stop} />}
        <MapToolbar
          level={camera.level}
          layersOpen={layersOpen}
          onZoom={camera.zoom}
          onCenterPlayer={() => translateTo(worldToGrid(pose.current.x, pose.current.z), CAMERA_CENTER_SECONDS)}
          onCenterHome={showHome}
          onToggleLayers={() => setLayersOpen((v) => !v)}
        />
        {gridAge != null && (
          <div className={noticeStyles.age}>
            <MaskIcon src={timeIcon} size={14} />
            {gridAge}
          </div>
        )}
        {layersOpen && <LayersPanel />}
        <div className={noticeStyles.credits}>Powered by the Decentraland Foundation</div>

        <RightPanel
          snapshot={snap}
          compact={compact != null}
          onExpand={() => nav.expand()}
          onCollapse={() => nav.collapse()}
          searchBar={
            <SearchBar
              input={snap.input}
              onType={(text) => nav.typeText(text)}
              onSubmit={() => nav.submitSearch()}
              onClear={() => nav.clearInput()}
              onBack={() => void nav.back()}
            />
          }
        >
          {snap.section === 'search' && snap.results.shown && (
            <SearchResults
              results={snap.results}
              sort={snap.sort}
              onSort={(sort) => nav.setSort(sort)}
              onHover={(place, hovered) => nav.hoverResult(place, hovered)}
              onPick={(place) => nav.clickResult(place)}
              onPage={(delta) => nav.turnPage(delta)}
            />
          )}
          {snap.section === 'place' && snap.place != null && (
            <PlaceInfo
              key={snap.place.visit}
              view={snap.place}
              referrer={address}
              isHome={placeIsHome(snap.place.place, snap.place.origin)}
              navigating={isRouteTo(destination, snap.place.place)}
              onPlaceChange={(place) => nav.updatePlace(place)}
              onHome={map.canPinHome ? () => snap.place != null && toggleHome(snap.place.place, snap.place.origin) : undefined}
              onStartNavigation={() => navigation.start(snap.place?.place ?? null)}
              onStopNavigation={navigation.stop}
              onJumpIn={jumpInPlace}
              onOpenEvent={(event: EventData) => void nav.selectEvent(event, snap.place?.place ?? null)}
              onOpenPhoto={(photos, index) => setPhoto({ photos, index })}
            />
          )}
          {snap.section === 'event' && snap.event != null && (
            <EventInfo key={snap.event.visit} view={snap.event} referrer={address} onEventChange={(event) => nav.updateEvent(event)} onJumpIn={jumpInEvent} />
          )}
        </RightPanel>
      </div>

    </>
  )
  if (compact != null) return body
  return (
    <MainMenuShell active="map" profileName={p?.name} profilePicture={p?.picture} profileAddress={p?.address} profileClaimed={p?.hasClaimedName} onNavigate={onNavigate} onClose={map.toggle}>
      {body}
    </MainMenuShell>
  )
}
