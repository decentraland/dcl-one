// The full-screen map: Genesis City with its marker layers, the category chips, the controls
// and Layers panel, and the panel of search results, places and events on the right.

import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { isGenesisSatelliteView } from '@explorer-core/features/map/atlas'
import { homeStore, isHomePlace } from '@explorer-core/features/map/home'
import { isRoad, isWorldPlace, type MapEvent, type MapPlace, type PlacePhoto } from '@explorer-core/features/map/mapApi'
import { parseParcel, playerMarkerPosition, worldToGrid, type Vec2 } from '@explorer-core/features/map/mapCoords'
import { mapLayers } from '@explorer-core/features/map/mapLayersStore'
import type { EventMarker, PlaceMarker } from '@explorer-core/features/map/mapMarkers'
import { CAMERA_CENTER_SECONDS, CAMERA_MOVE_SECONDS } from '@explorer-core/features/map/mapZoom'
import { destinationParcel, navigation } from '@explorer-core/features/map/navigation'
import { scenePins, type ScenePin } from '@explorer-core/features/map/scenePins'
import { Navmap } from '@explorer-core/features/map/navmap'
import { showToast } from '../../design'
import { PhotoDetail } from '../gallery/PhotoDetail'
import { MainMenuShell } from '../menu/MainMenuShell'
import type { GalleryState, MapState, MinimapState, ProfileState } from '../session/useEngineSession'
import { CategoryStrip } from './CategoryStrip'
import { DestinationBanner } from './DestinationBanner'
import { LayersPanel } from './LayersPanel'
import { MapCanvas } from './MapCanvas'
import { MapToolbar } from './MapToolbar'
import { PinsLayer } from './PinsLayer'
import { RouteLayer } from './RouteLayer'
import { WorldWarning } from './WorldWarning'
import { EventInfo } from './panel/EventInfo'
import { PlaceInfo } from './panel/PlaceInfo'
import { RightPanel } from './panel/RightPanel'
import { SearchBar } from './panel/SearchBar'
import { SearchResults } from './panel/SearchResults'
import { useMapCamera } from './useMapCamera'
import { useMapLayerData } from './useMapLayerData'
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
  onNavigate
}: {
  map: MapState
  minimap: MinimapState
  profile: ProfileState
  gallery: GalleryState
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  const open = map.open
  const camera = useMapCamera()
  const layers = useSyncExternalStore(mapLayers.subscribe, mapLayers.getSnapshot)
  const nav = useMemo(() => new Navmap(), [])
  const snap = useSyncExternalStore(nav.subscribe, nav.getSnapshot)
  const home = useSyncExternalStore(homeStore.subscribe, homeStore.getSnapshot)
  const destination = useSyncExternalStore(navigation.subscribe, navigation.getSnapshot)
  const pins = useSyncExternalStore(scenePins.subscribe, scenePins.getSnapshot)
  const [layersOpen, setLayersOpen] = useState(false)
  const [selected, setSelected] = useState<string | null>(null)
  const [warning, setWarning] = useState(false)
  const [photo, setPhoto] = useState<{ photos: PlacePhoto[]; index: number } | null>(null)

  const genesis = isGenesisSatelliteView(minimap.satelliteView)
  const address = profile.data?.address
  const { pose } = minimap
  const data = useMapLayerData(open, layers, address)

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

  // the route is followed whether or not the map is open
  useEffect(() => {
    if (destination == null) return
    const timer = setInterval(() => {
      if (navigation.playerMoved(playerMarkerPosition(pose.current.x, pose.current.z))) showToast('You have arrived to your destination')
    }, ARRIVAL_CHECK_MS)
    return () => clearInterval(timer)
  }, [destination, pose])

  const close = map.toggle

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
  const onPin = useCallback(
    (pin: ScenePin): void => {
      setSelected(null)
      // the place under the pin opens fresh, then again as a parcel picked on the map
      void nav.selectParcel(pin.parcel, false).then(() => nav.selectParcel(pin.parcel, true))
    },
    [nav]
  )
  const showHome = useCallback((): void => {
    if (home.world != null) return
    const parcel = home.parcel ?? { x: 0, y: 0 }
    setSelected(null)
    void nav.selectParcelAsHome(parcel)
  }, [home, nav])

  if (!open) return null

  const jumpInPlace = (): void => {
    const view = snap.place
    if (view == null) return
    const { place, origin, landOnOrigin } = view
    const base = parseParcel(place.base_position)
    const target = (landOnOrigin || isRoad(place.title)) && origin != null ? origin : base
    if (isWorldPlace(place)) {
      if (place.world_name == null) return
      nav.clearInput()
      map.changeRealm(place.world_name)
    } else {
      if (target == null) return
      if (destination != null && base != null && destination.parcel.x === base.x && destination.parcel.y === base.y) navigation.stop()
      nav.clearInput()
      map.teleportToPlace(target.x, target.y)
    }
    close()
  }
  const jumpInEvent = (): void => {
    const view = snap.event
    if (view == null) return
    nav.clearInput()
    map.teleportToPlace(view.event.x, view.event.y)
    close()
  }
  const setHome = (place: MapPlace, origin: Vec2 | null, on: boolean): void => {
    if (!on) {
      homeStore.set(null)
      return
    }
    if (isWorldPlace(place)) {
      if (place.world_name != null) homeStore.set({ world: place.world_name })
      return
    }
    const parcel = isRoad(place.title) && origin != null ? origin : parseParcel(place.base_position)
    if (parcel != null) homeStore.set({ parcel })
  }

  const placeIsHome = (place: MapPlace, origin: Vec2 | null): boolean =>
    isHomePlace(home, place) || (isRoad(place.title) && origin != null && home.world == null && home.parcel?.x === origin.x && home.parcel.y === origin.y)

  const navigatingTo = (place: MapPlace): boolean => {
    if (destination == null || isWorldPlace(place)) return false
    const end = destinationParcel(place)
    return destination.parcel.x === end.x && destination.parcel.y === end.y
  }

  const p = profile.data
  const openPhoto = photo != null ? photo.photos[photo.index] : undefined
  return (
    <MainMenuShell active="map" profileName={p?.name} profilePicture={p?.picture} profileAddress={p?.address} profileClaimed={p?.hasClaimedName} onNavigate={onNavigate} onClose={close}>
      <div className={styles.wrap}>
        <MapCanvas
          camera={camera}
          open={open}
          layers={layers}
          genesis={genesis}
          pose={pose}
          liveEvents={data.liveEvents}
          pois={data.pois}
          people={data.people}
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
        >
          <RouteLayer destination={destination} pose={pose} active={open} />
          {layers.has('pins') && <PinsLayer pins={pins} onPick={onPin} />}
        </MapCanvas>

        <CategoryStrip selected={snap.chip} onSelect={(key) => nav.selectCategory(key)} />
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
        {layersOpen && <LayersPanel />}
        <div className={noticeStyles.credits}>Powered by the Decentraland Foundation</div>

        <RightPanel
          snapshot={snap}
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
              navigating={navigatingTo(snap.place.place)}
              onPlaceChange={(place) => nav.updatePlace(place)}
              onHome={(on) => snap.place != null && setHome(snap.place.place, snap.place.origin, on)}
              onStartNavigation={() => navigation.start(snap.place?.place ?? null)}
              onStopNavigation={navigation.stop}
              onJumpIn={jumpInPlace}
              onOpenEvent={(event: MapEvent) => void nav.selectEvent(event, snap.place?.place ?? null)}
              onOpenPhoto={(photos, index) => setPhoto({ photos, index })}
            />
          )}
          {snap.section === 'event' && snap.event != null && (
            <EventInfo key={snap.event.visit} view={snap.event} referrer={address} onEventChange={(event) => nav.updateEvent(event)} onJumpIn={jumpInEvent} />
          )}
        </RightPanel>
      </div>

      {photo != null && openPhoto != null && (
        <PhotoDetail
          photos={photo.photos.map((ph) => ({ id: ph.id, url: ph.url, thumbnailUrl: ph.thumbnailUrl, dateTime: ph.dateTime ?? '' }))}
          index={photo.index}
          meta={gallery.metas[openPhoto.id]}
          isSelf={false}
          onIndex={(index) => setPhoto({ photos: photo.photos, index })}
          onLoadMeta={gallery.loadPhoto}
          onClose={() => setPhoto(null)}
          onTeleport={(x, y) => {
            setPhoto(null)
            nav.clearInput()
            map.teleportToPlace(x, y)
            close()
          }}
        />
      )}
    </MainMenuShell>
  )
}
