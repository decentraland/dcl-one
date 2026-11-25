import type { Entity, IEngine } from '@dcl/ecs';
import type { DeepReadonlyObject, PBVideoPlayer } from '@dcl/ecs';
// oxlint-disable-next-line no-unused-vars -- ReactEcs is the JSX factory
import ReactEcs, { Label, UiEntity } from '@dcl/react-ecs';
import { copyToClipboard } from '~system/RestrictedActions';
import { Button } from '../../Button';
import type { StreamKey } from '@dcl/sdk/cast';
import { LIVEKIT_STREAM_SRC } from '../../../definitions';
import { createVideoPlayerControls } from '../utils';
import { COLORS, RADIUS, SPACING, TYPE } from '../../theme';
import { Surface, Divider } from '../../Primitives';
import { CopyRow, ActionLink } from '../../Controls';
import { icon } from '../../icons';
import { ask, openSharePresentation, closeSharePresentation } from '../../actions';
import { getAdminMessageBus } from '../../admin-message-bus';
import { cast } from '../../cast';
import { VolumeSlider } from '../VolumeSlider';
import PresentationPanel from './PresentationPanel';

// Speakers showcase button — shown whenever the cast room is active, both while
// presenting and idle. flexGrow lets it sit beside "Share presentation" (half
// width) or stand alone (full width).
function SpeakersButton({ onShowShowcaseModal }: { onShowShowcaseModal: () => Promise<void> }) {
  return (
    <Button
      id="dcl_cast_showcase_list"
      value="<b>Speakers</b>"
      variant="secondary"
      fontSize={TYPE.body}
      color={COLORS.textTertiary}
      icon={icon('star')}
      iconTransform={{ width: 16, height: 16, margin: { right: SPACING.sm } }}
      iconBackground={{ color: COLORS.textTertiary }}
      uiTransform={{
        flexGrow: 1,
        flexBasis: 0,
        minWidth: 0,
        height: 40,
        borderRadius: RADIUS.md,
        alignItems: 'center',
        justifyContent: 'center',
      }}
      onMouseDown={onShowShowcaseModal}
    />
  );
}

const DclCastInfo = ({
  links,
  engine,
  onShowShowcaseModal,
  entity,
  video,
}: {
  links: NonNullable<StreamKey['links']>;
  engine: IEngine;
  onShowShowcaseModal: () => Promise<void>;
  entity: Entity;
  video: DeepReadonlyObject<PBVideoPlayer> | undefined;
}) => {
  const controls = createVideoPlayerControls(entity, engine);
  const active = getAdminMessageBus().source(entity) === 'dcl-cast';
  const following = cast().state;
  const show = following.tag === 'following' ? following.show : { tag: 'none' as const };

  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <Surface>
        <CopyRow
          id="dcl_cast_copy_stream_link"
          badge="mic"
          badgeVariant="magenta"
          title="Speaker link"
          description="Grants streaming access"
          onCopy={() => copyToClipboard({ text: links.stream })}
        />
        <UiEntity
          uiTransform={{ width: '100%', height: 1 }}
          uiBackground={{ color: COLORS.border }}
        />
        <CopyRow
          id="dcl_cast_copy_watcher_link"
          badge="eye"
          badgeVariant="blue"
          title="Viewer link"
          description="Grants viewing access"
          onCopy={() => copyToClipboard({ text: links.watch })}
        />
      </Surface>

      <VolumeSlider
        engine={engine}
        entity={entity}
        video={video}
      />

      {/* Speakers stays available whether presenting or idle. */}
      <UiEntity
        uiTransform={{ flexDirection: 'column', width: '100%', margin: { top: SPACING.xl } }}
      >
        {show.tag !== 'none' ? (
          <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
            <PresentationPanel show={show} />
            <UiEntity
              uiTransform={{ flexDirection: 'row', width: '100%', margin: { top: SPACING.md } }}
            >
              <SpeakersButton onShowShowcaseModal={onShowShowcaseModal} />
            </UiEntity>
          </UiEntity>
        ) : (
          <UiEntity
            uiTransform={{
              flexDirection: 'row',
              width: '100%',
              display: active ? 'flex' : 'none',
            }}
          >
            <Button
              id="dcl_cast_share_presentation_id"
              value="<b>Share presentation</b>"
              variant="primary"
              fontSize={TYPE.body}
              icon={icon('presentation')}
              iconTransform={{ width: 16, height: 16, margin: { right: SPACING.sm } }}
              iconBackground={{ color: COLORS.white }}
              uiTransform={{
                flexGrow: 1,
                flexBasis: 0,
                minWidth: 0,
                height: 40,
                borderRadius: RADIUS.md,
                alignItems: 'center',
                justifyContent: 'center',
                margin: { right: SPACING.md },
              }}
              onMouseDown={() => openSharePresentation(closeSharePresentation)}
            />
            <SpeakersButton onShowShowcaseModal={onShowShowcaseModal} />
          </UiEntity>
        )}
      </UiEntity>

      <UiEntity
        uiTransform={{ flexDirection: 'column', width: '100%', margin: { top: SPACING.xl } }}
      >
        <Divider />
        <UiEntity
          uiTransform={{
            flexDirection: 'row',
            alignItems: 'center',
            justifyContent: 'space-between',
            width: '100%',
            margin: { top: SPACING.xl },
          }}
        >
          <ActionLink
            label={active ? 'Deactivate room' : 'Activate room'}
            iconName="power"
            color={COLORS.textSecondary}
            onClick={() => controls.setSource(active ? '' : LIVEKIT_STREAM_SRC, 'dcl-cast')}
          />
          <ActionLink
            label="Reset room"
            iconName="refresh"
            color={COLORS.danger}
            onClick={() => ask('room')}
          />
        </UiEntity>
      </UiEntity>
    </UiEntity>
  );
};

export default DclCastInfo;
