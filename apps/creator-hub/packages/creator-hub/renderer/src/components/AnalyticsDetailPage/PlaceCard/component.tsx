import { useCallback, useState } from 'react';
import Button from '@ui/atoms/Button';
import Text from '@ui/atoms/Text';
import Tooltip from '@ui/atoms/Tooltip';

import {
  ArrowForwardIcon,
  CalendarMonthIcon as CalendarIcon,
  EditIcon,
  LinkIcon,
  LockIcon,
  PlaceIcon,
  PublicIcon,
  ThumbUpAltIcon as ThumbUpIcon,
} from '@ui/atoms/icons-material';

import type { PlaceDetails } from '/shared/types/place-analytics';
import { PlaceAccess } from '/shared/types/place-analytics';

import { misc } from '#preload';
import { useSnackbar } from '/@/hooks/useSnackbar';
import { t } from '/@/modules/store/translation/utils';
import { CREATE_EVENT_URL, getSceneJumpInDeeplink, getSceneJumpInUrl } from '/@/modules/utils';

import { formatDateTime, formatPercentage } from '../../AnalyticsPage/utils';

import './styles.css';

type Props = {
  place: PlaceDetails;
};

export function PlaceCard({ place }: Props) {
  const { pushGeneric } = useSnackbar();
  const jumpInUrl = getSceneJumpInUrl(place.location);
  /** Holds the URL that failed, so a new URL gets a new try. */
  const [failedAvatar, setFailedAvatar] = useState<string | null>(null);
  const avatar = place.lastPublishedBy?.avatar;

  const handleCreateEvent = useCallback(() => {
    void misc.openExternal(CREATE_EVENT_URL);
  }, []);

  const handleCopyUrl = useCallback(() => {
    void misc.copyToClipboard(jumpInUrl);
    pushGeneric('success', t('snackbar.generic.url_copied'));
  }, [jumpInUrl, pushGeneric]);

  /** Opens the desktop client itself; Copy URL keeps the shareable web link. */
  const handleJumpIn = useCallback(() => {
    void misc.openExternal(getSceneJumpInDeeplink(place.location));
  }, [place.location]);

  return (
    <div className="PlaceCard">
      <img
        className="Thumbnail"
        src={place.thumbnail}
        alt=""
      />
      <div className="Details">
        <Text
          variant="h5"
          className="Name"
        >
          {place.name}
        </Text>
        {place.likeRate !== null && (
          <div className="LikeRate">
            <ThumbUpIcon fontSize="small" />
            <Text variant="body1">{formatPercentage(place.likeRate)}</Text>
          </div>
        )}
        <div className="Field">
          <Text
            variant="body2"
            className="FieldLabel"
          >
            {t('analytics.detail.place.access')}
          </Text>
          <div className="FieldValue">
            {place.access === PlaceAccess.PRIVATE ? (
              <LockIcon fontSize="small" />
            ) : (
              <PublicIcon fontSize="small" />
            )}
            <Text variant="body1">
              {place.access === PlaceAccess.PRIVATE
                ? t('analytics.detail.place.private')
                : t('analytics.detail.place.public')}
            </Text>
          </div>
        </div>
        <div className="Field">
          <Text
            variant="body2"
            className="FieldLabel"
          >
            {t('analytics.detail.place.published_in')}
          </Text>
          <div className="FieldValue">
            <PlaceIcon fontSize="small" />
            <Text variant="body1">{place.publishedIn}</Text>
          </div>
        </div>
        {place.lastPublishedBy && (
          <div className="Field">
            <Text
              variant="body2"
              className="FieldLabel"
            >
              {t('analytics.detail.place.last_published_by')}
            </Text>
            <div className="FieldValue">
              <span className="PublisherAvatar">
                {avatar && avatar !== failedAvatar ? (
                  <img
                    src={avatar}
                    alt=""
                    onError={() => setFailedAvatar(avatar)}
                  />
                ) : (
                  place.lastPublishedBy.name.charAt(0)
                )}
              </span>
              <Text variant="body1">{place.lastPublishedBy.name}</Text>
            </div>
          </div>
        )}
        <div className="Field">
          <Text
            variant="body2"
            className="FieldLabel"
          >
            {t('analytics.detail.place.last_update')}
          </Text>
          <Text variant="body1">{formatDateTime(place.lastUpdatedAt)}</Text>
        </div>
      </div>
      <div className="Actions">
        <Button
          variant="text"
          tone="neutral"
          className="Action"
          onClick={handleCreateEvent}
        >
          <CalendarIcon />
          {t('analytics.detail.actions.create_event')}
        </Button>
        {/* Enabled once a published Place can be matched to a local scene. */}
        <Tooltip
          label={t('analytics.detail.actions.edit_scene_unavailable')}
          side="bottom"
          portal
          keepInViewport
          snap
          tipClassName="AnalyticsTip"
        >
          <Button
            variant="text"
            tone="neutral"
            className="Action"
            disabled
          >
            <EditIcon />
            {t('analytics.detail.actions.edit_scene')}
          </Button>
        </Tooltip>
        <Button
          variant="text"
          tone="neutral"
          className="Action"
          onClick={handleCopyUrl}
        >
          <LinkIcon />
          {t('analytics.detail.actions.copy_url')}
        </Button>
      </div>
      <Button
        variant="primary"
        className="JumpIn"
        onClick={handleJumpIn}
        endIcon={<ArrowForwardIcon />}
      >
        {t('analytics.detail.actions.jump_in')}
      </Button>
    </div>
  );
}
