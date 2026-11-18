import { createElement } from 'react';
import type { ComponentProps } from 'react';
import { cx } from '/@/lib/cx';

import { Bar } from '../../ManagePage/ViewBar';
import { classed } from '../classed';

import styles from './ScenesTab.module.css';

function ScenesBar(props: ComponentProps<typeof Bar>) {
  return createElement(Bar, { ...props, className: cx(styles.scenesBar, props.className) });
}

const Actions = classed('td', styles.actions);
const Bulk = classed('div', styles.bulk);
const Deployer = classed('span', styles.deployer);
const Meta = classed('span', styles.meta);
const Root = classed('div', styles.root);
const SceneName = classed('span', styles.sceneName);
const Stats = classed('div', styles.stats);
const Table = classed('table', styles.table);
const UpdatedAt = classed('time', styles.updatedAt);

export { Actions, Bulk, Deployer, Meta, Root, SceneName, ScenesBar, Stats, Table, UpdatedAt };
