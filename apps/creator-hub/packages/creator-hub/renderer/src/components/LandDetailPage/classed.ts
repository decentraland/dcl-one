import { createElement } from 'react';
import type { ComponentProps, ElementType, JSX, ReactElement } from 'react';
import { cx } from '/@/lib/cx';

type Tag = keyof JSX.IntrinsicElements;

export type ClassedProps<T extends Tag> = ComponentProps<T> & { as?: ElementType };

/** An element that always has one class of a CSS Module. `as` changes the tag; `className` adds to the class. */
export function classed<T extends Tag>(tag: T, base: string) {
  function Classed(props: ClassedProps<T>): ReactElement {
    const { as, className, ...rest } = props as { as?: ElementType; className?: string };
    return createElement(as ?? tag, { ...rest, className: cx(base, className) });
  }
  Classed.displayName = `classed(${tag})`;
  return Classed;
}
