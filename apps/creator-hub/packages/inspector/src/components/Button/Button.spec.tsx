import React from 'react';
import { readFileSync, readdirSync } from 'fs';
import { join } from 'path';
import { parseSync } from 'oxc-parser';
import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { Button } from '.';

const SRC = join(__dirname, '../..');
const UI3_CLASS = /\.(btn[\w-]*|is-disabled)\b/g;

const sources = (extension: string) =>
  readdirSync(SRC, { recursive: true, encoding: 'utf8' })
    .filter(file => file.endsWith(extension))
    .map(file => ({ file, text: readFileSync(join(SRC, file), 'utf8') }));

const stylesheetsKeyedOnUi3Button = () =>
  sources('.css').filter(({ text }) => /\.btn\b/.test(text));

const renderedButtons = () => {
  const looks = (['primary', 'secondary', 'ghost', 'text'] as const).flatMap(variant =>
    (['sm', 'md', 'lg'] as const).map(size => (
      <Button
        variant={variant}
        size={size}
        tone={variant === 'text' ? 'neutral' : undefined}
      >
        Go
      </Button>
    )),
  );
  const gated = (
    <Button
      as="a"
      href="#"
      aria-disabled="true"
    >
      Go
    </Button>
  );
  return [...looks, gated].map(element => {
    const { container, unmount } = render(element);
    const node = container.firstElementChild as HTMLElement;
    const rendered = { classes: [...node.classList], tone: node.dataset.tone };
    unmount();
    return rendered;
  });
};

type JsxNode = { type?: string; name?: JsxNode | string; value?: JsxNode | string };

const buttonCallSites = () => {
  const found: Record<string, string>[] = [];
  const visit = (node: unknown): void => {
    if (Array.isArray(node)) return node.forEach(visit);
    if (!node || typeof node !== 'object') return;
    const element = node as JsxNode & { attributes?: JsxNode[] };
    if (element.type === 'JSXOpeningElement' && (element.name as JsxNode).name === 'Button') {
      const props: Record<string, string> = {};
      for (const attribute of element.attributes ?? []) {
        const value = attribute.value as JsxNode | undefined;
        if (attribute.type === 'JSXAttribute' && typeof value?.value === 'string')
          props[(attribute.name as JsxNode).name as string] = value.value;
      }
      found.push(props);
    }
    Object.values(node).forEach(visit);
  };
  for (const { file, text } of sources('.tsx')) visit(parseSync(file, text).program);
  return found;
};

describe('when the inspector styles the ui3 Button', () => {
  it('should key every stylesheet only on classes the ui3 Button renders', () => {
    const rendered = new Set(renderedButtons().flatMap(({ classes }) => classes));
    const stylesheets = stylesheetsKeyedOnUi3Button();
    const unknown = Object.fromEntries(
      stylesheets
        .map(({ file, text }) => [
          file,
          [...new Set([...text.matchAll(UI3_CLASS)].map(([, name]) => name))].filter(
            name => !rendered.has(name),
          ),
        ])
        .filter(([, names]) => names.length),
    );

    expect(stylesheets.map(({ file }) => file)).toContain(
      join('components', 'Button', 'Button.css'),
    );
    expect(unknown).toEqual({});
  });

  it('should render the neutral tone the text look keys on', () => {
    const text = renderedButtons().filter(({ tone }) => tone);

    expect(text.length).toBeGreaterThan(0);
    expect(text.every(({ tone }) => tone === 'neutral')).toBe(true);
  });

  it('should keep the blue info look applied by primary call sites', () => {
    const info = buttonCallSites().filter(({ className }) =>
      className?.split(' ').includes('info'),
    );

    expect(info.length).toBeGreaterThan(0);
    expect(info.map(({ variant }) => variant)).toEqual(info.map(() => 'primary'));
  });
});
