import React, { useCallback, useState } from 'react';
import { VscQuestion as QuestionIcon } from 'react-icons/vsc';
import { AiOutlineInfoCircle as InfoIcon } from 'react-icons/ai';
import { FiAlertTriangle as WarningIcon } from 'react-icons/fi';
import { cx } from '../../../lib/utils/cx';
import Tooltip from '@ui/atoms/Tooltip';

import type { Position, Props } from './types';

import './InfoTooltip.css';

type Side = 'top' | 'bottom' | 'left' | 'right';

const MAX_WIDTH = 250;

const getSide = (position: Position): Side => position.split(' ')[0] as Side;

const InfoTooltip: React.FC<Props> = ({
  className,
  text,
  link,
  trigger,
  type = 'info',
  position = 'right center',
  open,
  disabled = false,
  hoverable = true,
  hideOnScroll = true,
  openOnTriggerMouseEnter = true,
  closeOnTriggerClick = false,
  onOpen,
  onClose,
}) => {
  // The trigger is the anchor, so no wrapper element is added around it.
  const [anchor, setAnchor] = useState<HTMLSpanElement | null>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [isClosedByClick, setIsClosedByClick] = useState(false);

  const handleOpenChange = useCallback(
    (isOpen: boolean) => {
      setIsHovered(isOpen);
      if (isOpen) {
        onOpen && onOpen();
      } else {
        setIsClosedByClick(false);
        onClose && onClose();
      }
    },
    [onOpen, onClose],
  );

  const handleTriggerClick = useCallback(() => {
    if (closeOnTriggerClick && isHovered) setIsClosedByClick(true);
  }, [closeOnTriggerClick, isHovered]);

  const renderIconTrigger = useCallback(() => {
    switch (type) {
      case 'info':
        return <InfoIcon size={16} />;
      case 'warning':
        return <WarningIcon size={16} />;
      default:
        return <QuestionIcon size={16} />;
    }
  }, [type]);

  const renderContent = useCallback(() => {
    return (
      <>
        {text}{' '}
        {link ? (
          <a
            href={link}
            target="_blank"
          >
            Learn more
          </a>
        ) : null}
      </>
    );
  }, [text, link]);

  return (
    <>
      <span
        ref={setAnchor}
        className={cx('InfoTooltipTrigger', { hovered: isHovered })}
        onClick={handleTriggerClick}
      >
        {trigger ?? renderIconTrigger()}
      </span>
      <Tooltip
        anchor={anchor}
        content={renderContent()}
        tipClassName={cx('InfoTooltip', className)}
        side={getSide(position)}
        open={open}
        onOpenChange={handleOpenChange}
        disabled={disabled || !openOnTriggerMouseEnter || isClosedByClick}
        interactive={hoverable}
        closeOnScroll={hideOnScroll}
        maxWidth={MAX_WIDTH}
        keepInViewport
      />
    </>
  );
};

export default React.memo(InfoTooltip);
