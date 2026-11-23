import { type IEngine } from '@dcl/ecs';
import ReactEcs, { UiEntity, Dropdown } from '@dcl/react-ecs';
import { type State } from '../types';
import type { AdminTools } from '../../definitions';
import { COLORS, RADIUS, SPACING, TYPE } from '../theme';
import { SectionHeader, FieldLabel, ActivePill, Divider } from '../Primitives';
import { Segmented } from '../Controls';
import { selectVideoSubTab, selectVideoPlayer } from '../actions';
import { getAdminMessageBus } from '../admin-message-bus';
import { cast } from '../cast';
import { getVideoPlayers, useSelectedVideoPlayer } from './utils';
import { VideoControlURL } from './VideoUrl';
import { LiveStream } from './LiveStream';
import DclCast from './DclCast';

export const DEFAULT_VOLUME = 1;

type MediaSource = 'video-url' | 'dcl-cast' | 'live';

const SECTION_PADDING = {
  left: SPACING.xxl,
  right: SPACING.xxl,
  top: SPACING.xl,
  bottom: SPACING.xl,
};

export function VideoControl({
  engine,
  state,
}: {
  engine: IEngine;
  state: State;
  /** @deprecated Not read: the Cast state knows the player. */
  playerAddress?: string;
}) {
  const [selectedEntity, selectedVideo] = useSelectedVideoPlayer(engine) ?? [];
  const videoPlayers = getVideoPlayers(engine);
  const selected = state.videoControl.selectedTab;

  // What the screen plays is the scene's state, the same for every admin: the panel opens on its
  // section and goes to it when any admin changes it. A presentation is run from DCL Cast.
  const active = selectedEntity && getAdminMessageBus().source(selectedEntity);
  const following = cast().state;
  const presenting = following.tag === 'following' && following.show.tag !== 'none';
  ReactEcs.useEffect(() => {
    selectVideoSubTab(presenting ? 'dcl-cast' : (active ?? 'live'));
  }, [state.videoControl.selectedVideoPlayer, active, presenting]);

  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <UiEntity uiTransform={{ flexDirection: 'column', width: '100%', padding: SECTION_PADDING }}>
        <SectionHeader
          title="Video screens"
          right={active && <ActivePill />}
        />

        {videoPlayers.length > 1 && (
          <UiEntity
            uiTransform={{ flexDirection: 'column', width: '100%', margin: { top: SPACING.xl } }}
          >
            <FieldLabel text="Screen" />
            <Dropdown
              options={videoPlayers.map(
                (player: NonNullable<AdminTools['videoControl']['videoPlayers']>[0]) =>
                  player.customName,
              )}
              selectedIndex={state.videoControl.selectedVideoPlayer ?? 0}
              onChange={idx => selectVideoPlayer(idx)}
              textAlign="middle-left"
              fontSize={TYPE.body}
              color={COLORS.inputText}
              uiTransform={{
                width: '100%',
                height: 40,
                borderRadius: RADIUS.md,
                borderWidth: 1,
                borderColor: COLORS.inputBorder,
              }}
              uiBackground={{ color: COLORS.inputBackground }}
            />
          </UiEntity>
        )}

        <UiEntity
          uiTransform={{ flexDirection: 'column', width: '100%', margin: { top: SPACING.xl } }}
        >
          <FieldLabel text="Media source" />
          <Segmented<MediaSource>
            options={[
              { key: 'video-url', label: 'URL', icon: 'play' },
              { key: 'dcl-cast', label: 'DCL Cast', icon: 'tv' },
              { key: 'live', label: 'Stream', icon: 'broadcast' },
            ]}
            selected={selected}
            onSelect={selectVideoSubTab}
          />
        </UiEntity>
      </UiEntity>

      {selected && selectedEntity && (
        <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
          <Divider />
          <UiEntity
            uiTransform={{ flexDirection: 'column', width: '100%', padding: SECTION_PADDING }}
          >
            {selected === 'video-url' && (
              <VideoControlURL
                engine={engine}
                entity={selectedEntity}
                video={selectedVideo}
              />
            )}
            {selected === 'live' && (
              <LiveStream
                engine={engine}
                entity={selectedEntity}
                video={selectedVideo}
              />
            )}
            {selected === 'dcl-cast' && (
              <DclCast
                engine={engine}
                state={state}
                entity={selectedEntity}
                video={selectedVideo}
              />
            )}
          </UiEntity>
        </UiEntity>
      )}
    </UiEntity>
  );
}
