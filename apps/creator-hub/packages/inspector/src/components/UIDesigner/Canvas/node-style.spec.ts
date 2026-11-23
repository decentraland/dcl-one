import { describe, expect, it } from 'vitest';

import type { UINode } from '../shared/tree-model';
import { nodeStyle, textureStyle, tintMatrix } from './node-style';

const RED_HALF = { r: 1, g: 0, b: 0, a: 0.5 };
const URL = 'blob:http://localhost/texture';

const node = (uiBackground: Record<string, unknown>): UINode =>
  ({ entity: 1, type: 'UiEntity', name: 'n', uiBackground, children: [] }) as unknown as UINode;

describe('nodeStyle background', () => {
  it('should paint the colour of a node with no texture', () => {
    expect(nodeStyle(node({ color: RED_HALF })).backgroundColor).toBe('rgba(255, 0, 0, 0.5)');
  });

  it('should paint no colour under an image texture, since the colour tints it', () => {
    const style = nodeStyle(
      node({ color: RED_HALF, texture: { tex: { $case: 'texture', texture: { src: 'a.png' } } } }),
    );
    expect(style.backgroundColor).toBeUndefined();
  });
});

describe('textureStyle', () => {
  it('should draw the texture on the layer, stretched in stretch mode', () => {
    const style = textureStyle(URL, 2, undefined);
    expect(style.backgroundImage).toBe(`url("${URL}")`);
    expect(style.backgroundSize).toBe('100% 100%');
  });

  it('should centre the texture at its own size in center mode', () => {
    const style = textureStyle(URL, 1, undefined);
    expect(style.backgroundSize).toBe('auto');
    expect(style.backgroundPosition).toBe('center');
  });
});

describe('tintMatrix', () => {
  it('should scale red, green, blue and alpha by the tint, as the explorer multiplies them', () => {
    expect(tintMatrix(RED_HALF)).toBe('1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.5 0');
  });

  it('should need no filter without a tint or with an opaque white one', () => {
    expect(tintMatrix(undefined)).toBeUndefined();
    expect(tintMatrix({ r: 1, g: 1, b: 1, a: 1 })).toBeUndefined();
    expect(tintMatrix({ r: 1, g: 1, b: 1 })).toBeUndefined();
  });
});
