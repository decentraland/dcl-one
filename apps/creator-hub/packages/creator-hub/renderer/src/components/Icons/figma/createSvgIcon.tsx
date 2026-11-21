import type { ReactNode } from 'react';
import type { IconDrawing } from '@dcl/inspector/icons';
import type { IconProps } from '@ui/atoms/icons-material';

import styles from './createSvgIcon.module.css';

/** Binds one Figma icon to ui3's icon props; `fontSize="small"` renders the small drawing at 16px. */
export function createSvgIcon(path: ReactNode, displayName: string, small?: IconDrawing) {
  function Icon({ fontSize = 'medium', tone = 'inherit', title, className, ...rest }: IconProps) {
    const drawing = fontSize === 'small' ? small : undefined;
    return (
      <svg
        className={className ? `${styles.root} ${className}` : styles.root}
        viewBox={drawing?.viewBox ?? '0 0 24 24'}
        overflow={drawing ? undefined : 'visible'}
        data-size={fontSize}
        data-tone={tone}
        focusable="false"
        role={title ? 'img' : undefined}
        aria-label={title}
        aria-hidden={title ? undefined : true}
        {...rest}
      >
        {title ? <title>{title}</title> : null}
        {drawing ? drawing.path : path}
      </svg>
    );
  }
  Icon.displayName = `${displayName}Icon`;
  return Icon;
}
