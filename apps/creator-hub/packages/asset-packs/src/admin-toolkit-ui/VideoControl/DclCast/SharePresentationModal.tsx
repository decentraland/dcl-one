import ReactEcs, { Label, UiEntity } from '@dcl/react-ecs';
import { Modal } from '../../Modal';
import { Button } from '../../Button';
import { cast, refusal } from '../../cast';
import { TYPE } from '../../theme';
import { TextField } from '../../Primitives';
import { getDclCastColors } from './styles';

const PRESENT = ['present', 'slides'];

const SharePresentationModal = ({
  onClose,
}: {
  onClose: () => void;
  /** @deprecated Not read: the Cast state holds the key. */
  streamingKey?: string;
}) => {
  const colors = getDclCastColors();
  const [url, setUrl] = ReactEcs.useState('');
  const [sent, setSent] = ReactEcs.useState(false);
  const [empty, setEmpty] = ReactEcs.useState(false);
  const following = cast().state;
  const isLoading = following.tag === 'following' && PRESENT.includes(following.write?.of ?? '');
  const error = empty ? 'Please enter a URL' : sent ? refusal(following, PRESENT) : '';

  // The answer is the next state: the modal closes when the presentation started.
  ReactEcs.useEffect(() => {
    if (sent && !isLoading && !error) onClose();
  }, [sent, isLoading, error]);

  const handleShare = () => {
    setEmpty(!url.trim());
    if (!url.trim()) return;
    cast().send({ do: 'present', file: url.trim() });
    setSent(true);
  };

  return (
    <Modal
      id="share-presentation"
      title="Share Presentation"
      onClose={onClose}
      width={480}
    >
      <UiEntity
        uiTransform={{
          flexDirection: 'column',
          width: '100%',
          margin: { top: 16 },
        }}
      >
        <Label
          value="Paste a URL to a PDF or PPTX file"
          fontSize={TYPE.body}
          color={colors.lightGray}
          uiTransform={{ margin: { bottom: 8 } }}
        />
        <TextField
          onChange={value => {
            setEmpty(false);
            setSent(false);
            setUrl(value);
          }}
          value={url}
          placeholder="https://example.com/slides.pdf"
          invalid={!!error}
          uiTransform={{ width: '100%' }}
        />
        {!!error && (
          <Label
            value={error}
            fontSize={TYPE.caption}
            color={colors.danger}
            uiTransform={{ margin: { top: 8 } }}
          />
        )}
        <UiEntity
          uiTransform={{
            flexDirection: 'row',
            justifyContent: 'flex-end',
            width: '100%',
            margin: { top: 16 },
          }}
        >
          <Button
            id="share_presentation_cancel"
            value="<b>Cancel</b>"
            variant="secondary"
            fontSize={TYPE.button}
            color={colors.white}
            uiTransform={{
              height: 42,
              padding: { left: 16, right: 16 },
              margin: { right: 8 },
            }}
            onMouseDown={onClose}
          />
          <Button
            id="share_presentation_submit"
            value={isLoading ? '<b>Sharing...</b>' : '<b>Share</b>'}
            fontSize={TYPE.button}
            disabled={isLoading}
            uiTransform={{
              height: 42,
              padding: { left: 16, right: 16 },
            }}
            onMouseDown={handleShare}
          />
        </UiEntity>
      </UiEntity>
    </Modal>
  );
};

export default SharePresentationModal;
