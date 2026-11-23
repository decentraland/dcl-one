import type { IEngine } from '@dcl/ecs';
// oxlint-disable-next-line no-unused-vars -- ReactEcs is the JSX factory
import ReactEcs, { UiEntity, Label } from '@dcl/react-ecs';
import { Button } from '../Button';
import { Error } from '../Error';
import { LoadingDots } from '../Loading';
import { COLORS, SPACING, TYPE } from '../theme';
import { state } from '../store';
import { ask } from '../actions';
import { ASK, cast, type KeyView } from '../cast';

const BUTTON = { height: 40, padding: { left: 16, right: 16 } };

/** What both sections show around the key: a notice, the wait, the way to a key, a refusal. */
export function KeyStatus({
  engine,
  view,
  get,
  onGet,
}: {
  engine: IEngine;
  view: KeyView;
  /** The label of the button that gets a key, where the section offers one. */
  get: string | false;
  onGet(): void;
}) {
  return (
    <UiEntity uiTransform={{ flexDirection: 'column', alignItems: 'center', width: '100%' }}>
      {!!view.notice && (
        <Label
          value={view.notice}
          fontSize={TYPE.label}
          color={COLORS.danger}
          uiTransform={{ margin: { bottom: SPACING.lg } }}
        />
      )}
      {view.tag === 'loading' && (
        <LoadingDots
          uiTransform={{ minHeight: 120 }}
          engine={engine}
        />
      )}
      {!!get && (
        <Button
          id="video_control_live_generate_key"
          value={`<b>${get}</b>`}
          fontSize={TYPE.subtitle}
          uiTransform={{ ...BUTTON, margin: { top: SPACING.xl, bottom: SPACING.xl } }}
          onMouseDown={onGet}
        />
      )}
      {!!view.error && <Error text={view.error} />}
    </UiEntity>
  );
}

/** The open confirmation: it sends its command only on the second click. */
export function Confirm() {
  const { title, text, label, cmd } = ASK[state.videoControl.ask!];
  // Each row states its height: with free-size rows the web Explorer drew the panel higher than its content.
  const row = {
    width: '100%' as const,
    flexDirection: 'row' as const,
    justifyContent: 'center' as const,
  };
  return (
    <UiEntity uiTransform={{ flexDirection: 'column', width: '100%' }}>
      <UiEntity
        uiTransform={{ ...row, height: 24, margin: { top: 36 } }}
        uiText={{ value: `<b>${title}</b>`, fontSize: TYPE.subtitle, color: COLORS.textPrimary }}
      />
      <UiEntity
        uiTransform={{ ...row, height: 36, margin: { top: 6, bottom: 24 } }}
        uiText={{ value: text, fontSize: TYPE.body, color: COLORS.textSecondary, textWrap: 'wrap' }}
      />
      <UiEntity uiTransform={{ ...row, height: 40, margin: { bottom: 36 } }}>
        <Button
          id="stream_key_cancel_remove"
          value="<b>Cancel</b>"
          variant="secondary"
          color={COLORS.textPrimary}
          uiTransform={{ ...BUTTON, margin: { right: 16 } }}
          onMouseDown={() => ask(undefined)}
        />
        <Button
          id="stream_key_confirm_remove"
          value={`<b>${label}</b>`}
          uiTransform={BUTTON}
          onMouseDown={() => {
            cast().send(cmd);
            ask(undefined);
          }}
        />
      </UiEntity>
    </UiEntity>
  );
}
