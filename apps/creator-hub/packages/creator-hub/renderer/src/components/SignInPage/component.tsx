import { useCallback } from 'react';
import { useNavigate } from 'react-router';
import { cx } from '/@/lib/cx';
import Button from '@ui/atoms/Button';
import Text from '@ui/atoms/Text';
import { ChevronLeftOutlinedIcon as ChevronLeftOutlined } from '@ui/atoms/icons-material';
import { t } from '/@/modules/store/translation/utils';
import { useAuth } from '/@/hooks/useAuth';
import { Row } from '../Row';
import { Column } from '../Column';

import EditorImage from '/assets/images/editor.png';

import styles from './component.module.css';

import './styles.css';

export function SignInPage() {
  const navigate = useNavigate();
  const { cancelSignIn, reopenSignInDapp, copySignInUrl } = useAuth();

  const handleBack = useCallback(() => {
    cancelSignIn();
    navigate(-1);
  }, [cancelSignIn, navigate]);

  const reopenAnchor = useCallback(
    (content: string) => (
      <a
        className="action-link"
        role="button"
        tabIndex={0}
        onClick={reopenSignInDapp}
      >
        {content}
      </a>
    ),
    [reopenSignInDapp],
  );

  const copyAnchor = useCallback(
    (content: string) => (
      <a
        className="action-link"
        role="button"
        tabIndex={0}
        onClick={copySignInUrl}
      >
        {content}
      </a>
    ),
    [copySignInUrl],
  );

  return (
    <div
      className={cx('SignIn', styles.page)}
      data-testid="sign-in-page"
    >
      <div className="background"></div>
      <div className={cx('content', styles.content)}>
        <Button
          className={cx('back', styles.back)}
          variant="primary"
          onClick={handleBack}
        >
          <ChevronLeftOutlined /> {t('sign_in.back')}
        </Button>
        <Row className="content-row">
          <Column className="text-column">
            <Text
              variant="h3"
              className={styles.gutter}
            >
              {t('sign_in.content.title')}
            </Text>
            <Text variant="h6">{t('sign_in.content.body', { br: () => <br /> })}</Text>
            <Text
              className="reopen"
              variant="body1"
            >
              {t('sign_in.content.reopen', { a: reopenAnchor, b: copyAnchor })}
            </Text>
          </Column>
          <img
            className="illustration"
            src={EditorImage}
            width={744}
            height={729}
            alt="Decentraland Creator Hub illustration"
          />
        </Row>
      </div>
    </div>
  );
}
