import { useCallback, useState } from 'react';
import Button from '@ui/atoms/Button';
import Text from '@ui/atoms/Text';
import TextField from '@ui/atoms/TextField';

import { t } from '/@/modules/store/translation/utils';

import styles from './component.module.css';

/** Where the user pastes the code an agent's sign-in page shows when the browser can't hand the
 *  sign-in back to the app on its own. */
export function SignInCodeField({ onSubmit }: { onSubmit: (code: string) => void }) {
  const [code, setCode] = useState('');
  const submit = useCallback(() => {
    if (code.trim() === '') return;
    onSubmit(code);
    setCode('');
  }, [code, onSubmit]);
  return (
    <div className={styles.root}>
      <Text
        variant="caption"
        tone="secondary"
      >
        {t('editor.ai.setup.signin_code_hint')}
      </Text>
      <div className={styles.row}>
        <TextField
          className={styles.field}
          size="small"
          fullWidth
          value={code}
          placeholder={t('editor.ai.setup.signin_code_placeholder')}
          aria-label={t('editor.ai.setup.signin_code_placeholder')}
          onChange={e => setCode(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') submit();
          }}
        />
        <Button
          className={styles.submit}
          variant="text"
          tone="neutral"
          size="sm"
          disabled={code.trim() === ''}
          onClick={submit}
        >
          {t('editor.ai.setup.signin_code_submit')}
        </Button>
      </div>
    </div>
  );
}
