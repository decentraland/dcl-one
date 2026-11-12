import React from 'react';
import { readFileSync } from 'fs';
import { join } from 'path';
import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';
import Ui3Button from '@ui/atoms/Button';

import { Button } from '.';

const stylesheet = readFileSync(join(__dirname, 'Button.css'), 'utf8');

const renderedClasses = () => {
  const classes = new Set<string>();
  for (const variant of ['primary', 'secondary', 'ghost', 'text'] as const) {
    for (const size of ['sm', 'md', 'lg'] as const) {
      const { container, unmount } = render(
        <Button
          variant={variant}
          size={size}
        >
          Go
        </Button>,
      );
      container.querySelector('button')!.classList.forEach(name => classes.add(name));
      unmount();
    }
  }
  return classes;
};

describe('when the inspector renders a Button', () => {
  it('should render the ui3 Button', () => {
    expect(Button).toBe(Ui3Button);
  });

  it('should key its looks only on classes the ui3 Button renders', () => {
    const keyed = new Set(stylesheet.match(/\.btn[\w-]*/g));
    const rendered = renderedClasses();

    expect([...keyed].map(name => name.slice(1)).filter(name => !rendered.has(name))).toEqual([]);
  });
});
