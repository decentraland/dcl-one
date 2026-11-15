import type { HTMLAttributes } from 'react';
import Text, { type TextProps } from '@ui/atoms/Text';

import styles from './component.module.css';

function join(base: string, className?: string) {
  return className ? `${base} ${className}` : base;
}

export function Actions({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      {...rest}
      className={join(styles.actions, className)}
    />
  );
}

export function Description({ className, ...rest }: TextProps) {
  return (
    <Text
      tone="secondary"
      {...rest}
      className={join(styles.description, className)}
    />
  );
}

export function PageFallback({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      {...rest}
      className={join(styles.fallback, className)}
    />
  );
}
