// oxlint-disable-next-line no-unused-vars -- ReactEcs is the JSX factory
import ReactEcs, { Label, UiEntity } from '@dcl/react-ecs';
import type { Show } from '@dcl/sdk/cast';
import { Button } from '../../Button';
import { COLORS, RADIUS, SPACING, TYPE } from '../../theme';
import { icon } from '../../icons';
import { cast } from '../../cast';

const CONTROL = {
  flexGrow: 1,
  flexBasis: 0,
  minWidth: 0,
  height: 34,
  borderRadius: RADIUS.md,
  alignItems: 'center' as const,
  justifyContent: 'center' as const,
  padding: { left: 4, right: 4 },
  margin: { right: SPACING.md },
};
const ICON = { width: 15, height: 15 };

/**
 * A presentation that is on: the stop control always, the slide controls once the bot states the
 * slides (an explorer that does not deliver the bot's packets never reaches `showing`).
 */
const PresentationPanel = ({ show }: { show: Exclude<Show, { tag: 'none' }> }) => {
  const slides = show.tag === 'showing' ? show.slides : undefined;
  // The pause shows only while the bot says that the video plays: a presenter that plays no video
  // (its state stays `idle`) never offers a control that it does not obey.
  const playing = slides?.video === 'playing' || slides?.video === 'loading';
  const { send } = cast();

  const stop = (
    <Button
      id="dcl_cast_stop"
      onlyIcon
      variant="primary"
      icon={icon('stop')}
      iconTransform={ICON}
      iconBackground={{ color: COLORS.white }}
      uiTransform={{
        width: 36,
        height: 34,
        borderRadius: RADIUS.md,
        alignItems: 'center',
        justifyContent: 'center',
      }}
      onMouseDown={() => send({ do: 'stop' })}
    />
  );

  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <UiEntity
        uiTransform={{
          width: '100%',
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'space-between',
          margin: { bottom: SPACING.md },
        }}
      >
        <UiEntity uiTransform={{ flexDirection: 'row', alignItems: 'center' }}>
          <UiEntity
            uiTransform={{ width: 6, height: 6, borderRadius: 3, margin: { right: 5 } }}
            uiBackground={{ color: COLORS.success }}
          />
          <Label
            value="Presenting"
            fontSize={TYPE.label}
            color={COLORS.success}
          />
        </UiEntity>
        {slides ? (
          <Label
            value={`Slide ${slides.slide + 1} of ${slides.count}`}
            fontSize={TYPE.label}
            color={COLORS.textSecondary}
          />
        ) : (
          stop
        )}
      </UiEntity>

      {slides && (
        <UiEntity uiTransform={{ flexDirection: 'row', alignItems: 'center', width: '100%' }}>
          <Button
            id="dcl_cast_prev"
            value="Prev"
            variant="secondary"
            fontSize={TYPE.body}
            color={COLORS.textTertiary}
            icon={icon('arrowL')}
            iconTransform={{ ...ICON, margin: { right: 5 } }}
            iconBackground={{ color: COLORS.textTertiary }}
            uiTransform={CONTROL}
            onMouseDown={() => send({ do: 'slide', to: 'prev' })}
          />
          <Button
            id="dcl_cast_next"
            value="Next"
            variant="secondary"
            fontSize={TYPE.body}
            color={COLORS.textTertiary}
            iconRight={icon('arrowR')}
            iconRightTransform={{ ...ICON, margin: { left: 5 } }}
            iconRightBackground={{ color: COLORS.textTertiary }}
            uiTransform={CONTROL}
            onMouseDown={() => send({ do: 'slide', to: 'next' })}
          />
          <Button
            id="dcl_cast_play"
            value={playing ? 'Pause video' : 'Play video'}
            variant="secondary"
            fontSize={TYPE.body}
            color={COLORS.textTertiary}
            disabled={!slides.videos}
            icon={icon(playing ? 'pause' : 'play')}
            iconTransform={{ ...ICON, margin: { right: 5 } }}
            iconBackground={{ color: COLORS.textTertiary }}
            uiTransform={CONTROL}
            onMouseDown={() => send(playing ? { do: 'pause' } : { do: 'play', video: 0 })}
          />
          {stop}
        </UiEntity>
      )}
    </UiEntity>
  );
};

export default PresentationPanel;
