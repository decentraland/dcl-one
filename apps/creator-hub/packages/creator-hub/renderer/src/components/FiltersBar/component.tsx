import { cx } from '/@/lib/cx';

import { Row } from '../Row';

import type { Props } from './types';

import './styles.css';

export function FiltersBar({ children, className }: Props) {
  const [left, right] = children;

  return (
    <Row className={cx('Filters', className)}>
      <Row className="left">{left}</Row>
      <Row className="right">{right}</Row>
    </Row>
  );
}
