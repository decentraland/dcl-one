import React from 'react';
import Button from '@ui/atoms/Button';
import Text from '@ui/atoms/Text';
import { t } from '/@/modules/store/translation/utils';
import './styles.css';

type Props = {
  onClickSignIn: () => void;
  /** Why this page needs an account, in its own words. */
  title: string;
};

const SignInCard: React.FC<Props> = ({ onClickSignIn, title }) => {
  return (
    <div className="Card SignInCard">
      <div className="CardContent CenteredContent">
        <Text
          className="CardTitle"
          variant="h6"
        >
          {title}
        </Text>
        <Button
          className="SignInButton"
          variant="primary"
          onClick={onClickSignIn}
        >
          {t('manage.sign_in.action')}
        </Button>
      </div>
    </div>
  );
};

export { SignInCard };
