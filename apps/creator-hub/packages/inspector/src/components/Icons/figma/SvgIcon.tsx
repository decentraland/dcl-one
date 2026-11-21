import React, { forwardRef, type SVGProps } from 'react';
import { cx } from '../../../lib/utils/cx';

import styles from './SvgIcon.module.css';

export type IconFontSize = 'inherit' | 'small' | 'medium' | 'large';

export type IconTone = 'inherit' | 'primary' | 'secondary' | 'error' | 'disabled';

export type SvgIconProps = Omit<SVGProps<SVGSVGElement>, 'fontSize' | 'title' | 'ref'> & {
  fontSize?: IconFontSize;
  tone?: IconTone;
  title?: string;
};

const capitalize = (value: string) => value[0].toUpperCase() + value.slice(1);

/** The inspector's icon base: ui3's icon props, sized and coloured by `SvgIcon.module.css`. */
export const SvgIcon = forwardRef<SVGSVGElement, SvgIconProps>(function SvgIcon(
  {
    fontSize = 'medium',
    tone = 'inherit',
    title,
    viewBox = '0 0 24 24',
    className,
    children,
    ...props
  },
  ref,
) {
  return (
    <svg
      ref={ref}
      viewBox={viewBox}
      fill="currentColor"
      focusable="false"
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
      {...props}
      className={cx(
        styles.SvgIcon,
        styles[`SvgIcon--fontSize${capitalize(fontSize)}`],
        tone !== 'inherit' && styles[`SvgIcon--tone${capitalize(tone)}`],
        className,
      )}
    >
      {title ? <title>{title}</title> : null}
      {children}
    </svg>
  );
});
