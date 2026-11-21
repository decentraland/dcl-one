import React from 'react';

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
import type { IconFontSize } from '../../Icons/figma/SvgIcon';
import {
  classifyNode,
  soleComponentRef,
  type UINode,
  type UINodeType,
  type WidgetKind,
  type WidgetPreset,
} from './tree-model';

type NodeTypeIcon = typeof Container;

export interface WidgetDef {
  id: string;
  type: UINodeType;
  label: string;
  Icon: NodeTypeIcon;
  keywords?: string[];
  preset?: WidgetPreset;
}

export interface WidgetCategory {
  category: string;
  items: WidgetDef[];
}

/** Single source of truth for the widget set (palette, tree icons, add menu). */
export const WIDGET_CATALOG: WidgetCategory[] = [
  {
    category: 'Containers',
    items: [
      {
        id: 'fullscreen',
        type: 'UiEntity',
        label: 'Full Screen',
        Icon: FullScreen,
        preset: 'fullscreen',
        keywords: ['root', 'wrapper', 'stretch', 'fill', 'screen', 'full'],
      },
      {
        id: 'UiEntity',
        type: 'UiEntity',
        label: 'Container',
        Icon: Container,
        keywords: ['box', 'panel', 'div', 'group', 'layout', 'flex'],
      },
      {
        id: 'image',
        type: 'UiEntity',
        label: 'Image',
        Icon: Image,
        preset: 'image',
        keywords: ['picture', 'texture', 'sprite', 'photo'],
      },
    ],
  },
  {
    category: 'Text',
    items: [
      {
        id: 'Label',
        type: 'Label',
        label: 'Label',
        Icon: Label,
        keywords: ['text', 'caption', 'title'],
      },
      {
        id: 'Button',
        type: 'Button',
        label: 'Button',
        Icon: Button,
        keywords: ['click', 'action', 'cta'],
      },
    ],
  },
  {
    category: 'Input',
    items: [
      {
        id: 'Input',
        type: 'Input',
        label: 'Input',
        Icon: Input,
        keywords: ['text field', 'form', 'entry'],
      },
      {
        id: 'Dropdown',
        type: 'Dropdown',
        label: 'Dropdown',
        Icon: Dropdown,
        keywords: ['select', 'combo', 'options'],
      },
    ],
  },
];

/** Flat list of all widgets, derived from the catalog. */
export const WIDGET_LIST: WidgetDef[] = WIDGET_CATALOG.flatMap(c => c.items);

const KIND_ICONS = Object.fromEntries(WIDGET_LIST.map(w => [w.label, w.Icon])) as Record<
  WidgetKind,
  NodeTypeIcon
>;

/** A node's NodeType glyph from the icon library: Prefab for a nested GUI, else its widget kind's. */
export const NodeIcon: React.FC<{ node: UINode; fontSize: IconFontSize }> = ({
  node,
  fontSize,
}) => {
  const ref = (node as { componentRef?: unknown }).componentRef || soleComponentRef(node);
  const Icon = ref ? Prefab : KIND_ICONS[classifyNode(node)];
  return <Icon fontSize={fontSize} />;
};
