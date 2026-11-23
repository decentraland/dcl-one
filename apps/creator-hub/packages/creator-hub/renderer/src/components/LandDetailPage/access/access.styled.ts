import { createElement } from 'react';
import type { ComponentProps } from 'react';
import { cx } from '/@/lib/cx';

import { classed } from '../classed';
import { DomainSelect } from '../DetailsTab/DetailsTab.styled';

import styles from './access.module.css';

function ParcelSelect(props: ComponentProps<typeof DomainSelect>) {
  return createElement(DomainSelect, {
    ...props,
    className: cx(styles.parcelSelect, props.className),
  });
}

const AddBlock = classed('div', styles.addBlock);
const AddButton = classed('span', styles.addButton);
const AddRow = classed('div', styles.addRow);
const Description = classed('p', styles.description);
const ListBlock = classed('div', styles.listBlock);
const ListHeader = classed('div', styles.listHeader);
const Notice = classed('p', styles.notice);
const Root = classed('div', styles.root);
const Row = classed('li', styles.row);
const Rows = classed('ul', styles.rows);

export {
  AddBlock,
  AddButton,
  AddRow,
  Description,
  ListBlock,
  ListHeader,
  Notice,
  ParcelSelect,
  Root,
  Row,
  Rows,
};
