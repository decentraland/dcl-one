// oxlint-disable-next-line typescript/consistent-type-imports -- ReactEcs is the JSX factory
import ReactEcs, { UiEntity, Label } from '@dcl/react-ecs';
import type { DeepReadonlyObject, Entity, IEngine, PBVideoPlayer } from '@dcl/ecs';
import { copyToClipboard, openExternalUrl } from '~system/RestrictedActions';
import { LIVEKIT_STREAM_SRC } from '../../../definitions';
import { state } from '../../store';
import { docsUrl } from '../../fetch-utils';
import { ask, showKey } from '../../actions';
import { getAdminMessageBus } from '../../admin-message-bus';
import { cast, keyView, type KeyView } from '../../cast';
import { COLORS, RADIUS, SPACING, TYPE } from '../../theme';
import { SectionHeader, FieldLabel, Icon, Divider } from '../../Primitives';
import { CopyButton, ActionLink, PillButton } from '../../Controls';
import { createVideoPlayerControls } from '../utils';
import { VolumeSlider } from '../VolumeSlider';
import { Confirm, KeyStatus } from '../KeyStatus';

const STREAMING_HELP = '/creator/scene-editor/live-ops/live-streaming';
// Shown only when the gatekeeper's response names no ingress URL. The stream key belongs to the
// ingress the gatekeeper created it on, so its `streamingUrl` is the server to publish to.
const FALLBACK_RTMP_SERVER_URL = 'rtmps://dcl.rtmp.livekit.cloud/x';

type Props = {
  engine: IEngine;
  entity: Entity;
  video: DeepReadonlyObject<PBVideoPlayer> | undefined;
};

export function LiveStream({ engine, entity, video }: Props) {
  if (state.videoControl.ask) return <Confirm />;
  const view = keyView();

  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <SectionHeader
        title="Stream"
        right={
          <UiEntity
            uiTransform={{ width: 16, height: 16 }}
            uiBackground={{ color: COLORS.transparent }}
            onMouseDown={() => openExternalUrl({ url: docsUrl(STREAMING_HELP) })}
          >
            <Icon
              name="help"
              size={16}
              color={COLORS.textSecondary}
            />
          </UiEntity>
        }
      />
      <UiEntity
        uiTransform={{ width: '100%', height: 36, margin: { top: SPACING.lg, bottom: SPACING.xl } }}
        uiText={{
          value:
            'Use this RTMP server and stream key in your broadcasting software to stream to this screen.',
          fontSize: TYPE.label,
          color: COLORS.textSecondary,
          textAlign: 'top-left',
          textWrap: 'wrap',
        }}
      />
      <KeyStatus
        engine={engine}
        view={view}
        get={view.tag === 'none' && 'Get Stream Key'}
        onGet={() => cast().send({ do: 'generate' })}
      />
      {view.tag === 'key' && (
        <StreamKey
          engine={engine}
          entity={entity}
          video={video}
          view={view}
        />
      )}
    </UiEntity>
  );
}

// A read-only field (RTMP server / stream key) with an optional trailing slot.
function ReadonlyField({ value, trailing }: { value: string; trailing?: ReactEcs.JSX.Element }) {
  return (
    <UiEntity
      uiTransform={{
        flexGrow: 1,
        flexBasis: 0,
        minWidth: 0,
        height: 40,
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
        borderRadius: RADIUS.md,
        borderWidth: 1,
        borderColor: COLORS.inputBorder,
        padding: { left: SPACING.xl, right: SPACING.xl },
        margin: { right: SPACING.md },
      }}
      uiBackground={{ color: COLORS.inputBackground }}
    >
      <Label
        value={value}
        fontSize={TYPE.body}
        color={COLORS.textPrimary}
      />
      {trailing ? trailing : <UiEntity uiTransform={{ width: 0, height: 0 }} />}
    </UiEntity>
  );
}

function StreamKey({
  engine,
  entity,
  video,
  view: { key, expires, soon },
}: Props & { view: KeyView & { tag: 'key' } }) {
  const controls = createVideoPlayerControls(entity, engine);
  const rtmpServerUrl = key.url || FALLBACK_RTMP_SERVER_URL;
  const shown = Date.now() < state.videoControl.keyShownUntil;
  const isLive = getAdminMessageBus().source(entity) === 'live';

  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
        <FieldLabel text="RTMP server" />
        <UiEntity uiTransform={{ flexDirection: 'row', alignItems: 'center', width: '100%' }}>
          <ReadonlyField value={rtmpServerUrl} />
          <CopyButton
            id="video_control_copy_rtmp_server"
            onCopy={() => copyToClipboard({ text: rtmpServerUrl })}
          />
        </UiEntity>
      </UiEntity>

      <UiEntity
        uiTransform={{ flexDirection: 'column', width: '100%', margin: { top: SPACING.xl } }}
      >
        <FieldLabel text="Stream key" />
        <UiEntity uiTransform={{ flexDirection: 'row', alignItems: 'center', width: '100%' }}>
          <ReadonlyField
            value={shown ? key.key : '••••••••••••'}
            trailing={
              <UiEntity
                uiTransform={{ width: 16, height: 16 }}
                uiBackground={{ color: COLORS.transparent }}
                onMouseDown={() => showKey(!shown)}
              >
                <Icon
                  name={shown ? 'eyeoff' : 'eye'}
                  size={16}
                  color={COLORS.textSecondary}
                />
              </UiEntity>
            }
          />
          <CopyButton
            id="video_control_copy_stream_key"
            onCopy={() => copyToClipboard({ text: key.key })}
          />
        </UiEntity>
        <UiEntity
          uiTransform={{ width: '100%', height: 44, margin: { top: SPACING.sm } }}
          uiText={{
            value:
              'Do not share your stream key with anyone, and be careful not to display it on screen while streaming.',
            fontSize: TYPE.label,
            color: COLORS.danger,
            textAlign: 'top-left',
            textWrap: 'wrap',
          }}
        />
      </UiEntity>

      <UiEntity
        uiTransform={{
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'space-between',
          width: '100%',
          margin: { top: SPACING.xl },
        }}
      >
        <UiEntity uiTransform={{ flexDirection: 'row', alignItems: 'center' }}>
          <Icon
            name="clock"
            size={14}
            color={soon ? COLORS.danger : COLORS.textSecondary}
            uiTransform={{ margin: { right: 5 } }}
          />
          <Label
            value={`Stream expires in ${expires}`}
            fontSize={TYPE.label}
            color={soon ? COLORS.danger : COLORS.textSecondary}
          />
        </UiEntity>
        <PillButton
          id="video_control_stream_activate"
          label={isLive ? 'Deactivate' : 'Activate'}
          variant="filled"
          onClick={() => controls.setSource(isLive ? '' : LIVEKIT_STREAM_SRC, 'live')}
        />
      </UiEntity>

      <VolumeSlider
        engine={engine}
        entity={entity}
        video={video}
      />

      <Divider uiTransform={{ margin: { top: SPACING.xl } }} />
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
          label="Revoke stream key"
          iconName="trash"
          color={COLORS.danger}
          onClick={() => ask('revoke')}
        />
        <ActionLink
          label="Reset stream key"
          iconName="refresh"
          color={COLORS.danger}
          onClick={() => ask('reset')}
        />
      </UiEntity>
    </UiEntity>
  );
}
