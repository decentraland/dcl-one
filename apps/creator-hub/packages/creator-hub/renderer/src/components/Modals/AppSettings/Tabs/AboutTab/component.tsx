import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';
import { UpdateSettings } from '../../UpdateSettings';
import type { AboutTabProps } from '../../types';
import logo from '/assets/images/logo-editor.png';

import './styles.css';

const AboutTab: React.FC<AboutTabProps> = ({ version, onViewChangelog, autoCheckForUpdates }) => {
  return (
    <div className="AboutTabContainer">
      <div className="AboutHeader">
        <img
          src={logo}
          alt="Decentraland Creator Hub"
          className="AboutLogo"
        />
        <div className="AboutInfo">
          <Text
            variant="h5"
            className="AboutTitle"
          >
            {t('modal.app_settings.about.title')}
          </Text>
          {version && (
            <div className="AboutVersionRow">
              <Text variant="body2">{`v${version}`}</Text>
              <button
                type="button"
                onClick={onViewChangelog}
                className="AboutChangelogLink"
              >
                {t('modal.app_settings.about.view_changelog')}
              </button>
            </div>
          )}
        </div>
      </div>
      <div className="AboutUpdateSection">
        <UpdateSettings
          className="AboutUpdateSettings"
          autoCheck={autoCheckForUpdates}
        />
      </div>
    </div>
  );
};

export default AboutTab;
