// oxlint-disable-next-line no-unused-vars -- ReactEcs is the JSX factory
import ReactEcs, { UiEntity } from '@dcl/react-ecs';
import type { DeepReadonlyObject, Entity, IEngine, PBVideoPlayer } from '@dcl/ecs';
import { LIVEKIT_STREAM_SRC } from '../../../definitions';
import type { State } from '../../types';
import { clearInterval, setInterval } from '../../utils';
import { getActiveStreams, groupTracksByParticipant, type FlattenedTrack } from '../api';
import { createVideoPlayerControls } from '../utils';
import { openShowcase, closeShowcase, setParticipants } from '../../actions';
import { cast, keyView, needsLinks } from '../../cast';
import { COLORS, SPACING, TYPE } from '../../theme';
import { SectionHeader, Icon } from '../../Primitives';
import { Confirm, KeyStatus } from '../KeyStatus';
import DclCastInfo from './DclCastInfo';

const DclCast = ({
  engine,
  state,
  entity,
  video,
}: {
  engine: IEngine;
  state: State;
  entity: Entity;
  video: DeepReadonlyObject<PBVideoPlayer> | undefined;
}) => {
  if (state.videoControl.ask) return <Confirm />;
  const controls = createVideoPlayerControls(entity, engine);
  if (needsLinks()) cast().send({ do: 'link' });
  const view = keyView();
  const links = view.tag === 'key' && view.key.links;

  const onShowShowcaseModal = async () => {
    const refresh = async () => {
      const tracks = await getActiveStreams();
      if (tracks) setParticipants(groupTracksByParticipant(tracks));
    };
    await refresh();
    const timer = setInterval(engine, refresh, 1000);
    openShowcase({
      onSelectTrack: (track: FlattenedTrack) => controls.setSource(track.sid, 'dcl-cast'),
      onSetDefault: () => controls.setSource(LIVEKIT_STREAM_SRC, 'dcl-cast'),
      onClose: () => {
        clearInterval(engine, timer);
        closeShowcase();
      },
    });
  };

  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <SectionHeader
        title="DCL Cast room"
        right={
          view.tag === 'key' && (
            <UiEntity uiTransform={{ flexDirection: 'row', alignItems: 'center' }}>
              <Icon
                name="clock"
                size={13}
                color={COLORS.textSecondary}
                uiTransform={{ margin: { right: SPACING.xs } }}
              />
              <UiEntity
                uiText={{
                  value: `Expires in ${view.expires}`,
                  fontSize: TYPE.label,
                  color: COLORS.textSecondary,
                }}
              />
            </UiEntity>
          )
        }
      />
      <UiEntity
        uiTransform={{
          width: '100%',
          height: 36,
          margin: { top: SPACING.lg, bottom: SPACING.xl },
        }}
        uiText={{
          value: 'Stream camera and screen from the browser to a screen in your scene.',
          fontSize: TYPE.label,
          color: COLORS.textSecondary,
          textAlign: 'top-left',
          textWrap: 'wrap',
        }}
      />
      <KeyStatus
        engine={engine}
        view={view}
        get={(view.tag === 'none' || (view.tag === 'key' && !links)) && 'Get Cast links'}
        onGet={() => cast().send({ do: 'link' })}
      />
      {links && (
        <DclCastInfo
          links={links}
          entity={entity}
          engine={engine}
          video={video}
          onShowShowcaseModal={onShowShowcaseModal}
        />
      )}
    </UiEntity>
  );
};

export default DclCast;
