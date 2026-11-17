// Path data of the icons in this folder: @mui/icons-material 5.18.0 (MIT). The licence text is in ./LICENSE.
import { forwardRef, memo, type CSSProperties, type ReactNode, type SVGProps } from 'react';
import { cx } from '/@/lib/cx';

import styles from './SvgIcon.module.css';

/** The `sx` keys that callers use. Each key becomes a rule with the weight of one class, as before. */
export type SvgIconSx = {
  fontSize?: number | string;
  marginLeft?: 'auto';
  visibility?: 'visible' | 'hidden';
};

export type SvgIconProps = Omit<SVGProps<SVGSVGElement>, 'color' | 'fontSize' | 'ref'> & {
  fontSize?: 'inherit' | 'small' | 'medium' | 'large';
  color?: 'inherit' | 'primary' | 'secondary';
  /** Sets the `color` attribute of the svg element. */
  htmlColor?: string;
  /** Gives the icon a name for assistive technology. Without it the icon is hidden from them. */
  titleAccess?: string;
  sx?: SvgIconSx;
};

const SX_VARIABLES = [
  ['fontSize', '--svg-icon-font-size', styles.sxFontSize],
  ['marginLeft', '--svg-icon-margin-left', styles.sxMarginLeft],
  ['visibility', '--svg-icon-visibility', styles.sxVisibility],
] as const;

/** An icon as an svg element. Size comes from `font-size`, colour from `currentColor`. */
export const SvgIcon = forwardRef<SVGSVGElement, SvgIconProps>(function SvgIcon(
  {
    className,
    fontSize = 'medium',
    color = 'inherit',
    htmlColor,
    titleAccess,
    sx,
    style,
    children,
    ...other
  },
  ref,
) {
  const sxClasses: string[] = [];
  const sxStyle: Record<string, string> = {};
  for (const [key, variable, sxClass] of SX_VARIABLES) {
    const value = sx?.[key];
    if (value === undefined) continue;
    sxClasses.push(sxClass);
    sxStyle[variable] = typeof value === 'number' ? `${value}px` : value;
  }
  return (
    <svg
      // The renderer style sheets select icons by the `MuiSvgIcon-root` class, and so do the capture scenarios.
      className={cx(
        'MuiSvgIcon-root',
        styles.root,
        {
          [styles.fontSizeInherit]: fontSize === 'inherit',
          [styles.fontSizeMedium]: fontSize === 'medium',
          [styles.fontSizeSmall]: fontSize === 'small',
          [styles.fontSizeLarge]: fontSize === 'large',
          [styles.colorPrimary]: color === 'primary',
          [styles.colorSecondary]: color === 'secondary',
        },
        sxClasses,
        className,
      )}
      focusable="false"
      color={htmlColor}
      aria-hidden={titleAccess ? undefined : true}
      role={titleAccess ? 'img' : undefined}
      viewBox="0 0 24 24"
      ref={ref}
      {...other}
      style={sxClasses.length > 0 ? ({ ...sxStyle, ...style } as CSSProperties) : style}
    >
      {children}
      {titleAccess ? <title>{titleAccess}</title> : null}
    </svg>
  );
});

/** A 24x24 filled Material icon. */
export function createSvgIcon(path: ReactNode, name: string) {
  const testId = `${name}Icon`;
  const Icon = forwardRef<SVGSVGElement, SvgIconProps>(function Icon(props, ref) {
    return (
      <SvgIcon
        data-testid={testId}
        ref={ref}
        {...props}
      >
        {path}
      </SvgIcon>
    );
  });
  Icon.displayName = testId;
  return memo(Icon);
}
