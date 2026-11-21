import React from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import {
  Button,
  Container,
  Dropdown,
  FullScreen,
  Image,
  Input,
  Label,
  Prefab,
} from '../../Icons/figma';
import type { UINode } from './tree-model';
import { matchWidgets, NodeIcon, WIDGET_CATALOG, WIDGET_LIST } from './widget-catalog';

const svgOf = (ui: React.ReactElement) => {
  const html = render(ui).container.querySelector('svg')!.outerHTML;
  cleanup();
  return html;
};

const node = (over: Partial<UINode> & Record<string, unknown>): UINode =>
  ({ entity: 1, name: 'n', children: [], ...over }) as unknown as UINode;

describe('the widget catalog', () => {
  afterEach(cleanup);

  it.each([
    ['fullscreen', FullScreen],
    ['UiEntity', Container],
    ['image', Image],
    ['Label', Label],
    ['Button', Button],
    ['Input', Input],
    ['Dropdown', Dropdown],
  ])('should give %s its NodeType library icon', (id, Icon) => {
    expect(WIDGET_LIST.find(w => w.id === id)?.Icon).toBe(Icon);
  });
});

describe('NodeIcon', () => {
  afterEach(cleanup);

  it.each([
    ['a container', node({ type: 'UiEntity' }), Container],
    [
      'an image awaiting a texture',
      node({ type: 'UiEntity', uiBackground: { textureMode: 1 } }),
      Image,
    ],
    [
      'an image on a solid colour',
      node({
        type: 'UiEntity',
        uiBackground: { textureMode: 1, color: { r: 1, g: 0, b: 0, a: 1 } },
      }),
      Image,
    ],
    ['a label', node({ type: 'Label' }), Label],
    ['a nested GUI', node({ type: 'UiEntity', componentRef: { name: 'Card' } }), Prefab],
  ])('should draw %s with its library icon at the size asked', (_, n, Icon) => {
    expect(
      svgOf(
        <NodeIcon
          node={n}
          fontSize="small"
        />,
      ),
    ).toBe(svgOf(<Icon fontSize="small" />));
    expect(
      svgOf(
        <NodeIcon
          node={n}
          fontSize="inherit"
        />,
      ),
    ).toBe(svgOf(<Icon fontSize="inherit" />));
  });
});

describe('matchWidgets', () => {
  const labels = (query: string) =>
    matchWidgets(query).map(c => [c.category, c.items.map(i => i.label)]);

  it('should return the whole catalog for an empty or blank query', () => {
    expect(matchWidgets('')).toEqual(WIDGET_CATALOG);
    expect(matchWidgets('   ')).toEqual(WIDGET_CATALOG);
  });

  it('should match a category name, its keywords and labels, case-insensitively', () => {
    expect(labels('Text')).toEqual([
      ['Containers', ['Image']],
      ['Text', ['Label', 'Button']],
      ['Input', ['Input']],
    ]);
  });

  it('should match a keyword alone', () => {
    expect(labels('box')).toEqual([['Containers', ['Container']]]);
  });

  it('should match a label in any case', () => {
    expect(labels('LAB')).toEqual([['Text', ['Label']]]);
  });

  it('should drop every category when nothing matches', () => {
    expect(matchWidgets('zzz')).toEqual([]);
  });
});
