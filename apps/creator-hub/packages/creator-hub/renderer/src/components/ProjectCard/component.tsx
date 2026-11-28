import { type MouseEvent, useCallback } from 'react';
import Spinner from '@ui/atoms/Spinner';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';
import { isTimeAgo, minutes } from '/shared/time';

import { Dropdown } from '../Dropdown';

import type { Props } from './types';

import './styles.css';

export function ProjectCard({
  title,
  description,
  imageUrl,
  videoUrl,
  content,
  dropdownOptions,
  width = 256,
  height = 240,
  publishedAt = 0,
  onClick,
  status,
  'data-testid': dataTestId,
}: Props) {
  const handleMouseEnterVideo = useCallback(
    ({ currentTarget: video }: React.MouseEvent<HTMLVideoElement>) => {
      video.muted = true;
      video.play();
    },
    [],
  );

  const handleMouseLeaveVideo = useCallback(
    ({ currentTarget: video }: React.MouseEvent<HTMLVideoElement>) => {
      // load() rather than pause + rewind: it is what brings the poster back.
      video.load();
    },
    [],
  );

  const widthPx = `${width}px`;
  const heightPx = `${height}px`;

  return (
    <div
      className="ProjectCard"
      data-testid={dataTestId}
      onClick={onClick}
      style={{ width: widthPx, height: heightPx }}
    >
      <Overlay
        status={status}
        width={widthPx}
        height={heightPx}
      />
      {videoUrl ? (
        <video
          className="video"
          src={videoUrl}
          poster={imageUrl}
          onMouseEnter={handleMouseEnterVideo}
          onMouseLeave={handleMouseLeaveVideo}
        />
      ) : (
        <div
          className="thumbnail"
          style={imageUrl ? { backgroundImage: `url(${imageUrl})` } : {}}
        />
      )}
      {isTimeAgo(publishedAt, minutes(10)) && (
        <span className="badge">{t('scene_list.badges.published')}</span>
      )}
      <div className="info">
        <div className="title">
          <Text variant="h6">{title}</Text>
          {!!dropdownOptions?.length && (
            <Dropdown
              label={t('menu.options_for', { name: title })}
              options={dropdownOptions}
            />
          )}
        </div>
        {description && <p className="description">{description}</p>}
        {content && <div className="content">{content}</div>}
      </div>
    </div>
  );
}

function Overlay({
  status,
  width,
  height,
}: {
  status: Props['status'];
  width: string;
  height: string;
}) {
  if (status !== 'loading') return null;

  const handleClick = useCallback((e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  return (
    <div
      className="Overlay"
      style={{ width, height }}
      onClick={handleClick}
    >
      <Spinner
        ring="wide"
        role="progressbar"
        size={40}
        color="primary"
      />
      {t('scene_list.generating_thumbnail')}
    </div>
  );
}
