import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import type { Entity, TextureUnion } from '@dcl/ecs';

import type * as UiBarrel from '../../../../ui';
import { FillField } from './FillField';

vi.mock('../../../../../hooks/useAssetOptions', () => ({
  useAssetOptions: () => [],
}));

vi.mock('../../../../ui', async importOriginal => ({
  ...(await importOriginal<typeof UiBarrel>()),
  FileUploadField: ({ label }: { label?: string }) => <div>{label}</div>,
}));

const WHITE = { r: 1, g: 1, b: 1, a: 1 };
const CLEAR = { r: 0, g: 0, b: 0, a: 0 };
const IMAGE_TEXTURE = {
  tex: { $case: 'texture', texture: { src: 'images/a.png' } },
} as TextureUnion;

function renderField(props: {
  color?: typeof WHITE;
  texture?: TextureUnion;
  textureMode?: number;
}) {
  const onPatch = vi.fn();
  render(
    <FillField
      color={props.color}
      texture={props.texture}
      textureMode={props.textureMode}
      entity={1 as unknown as Entity}
      onPatch={onPatch}
    />,
  );
  return { onPatch };
}

const checked = (label: string) => screen.getByLabelText(label).getAttribute('aria-checked');

describe('when the node has a colour but no texture', () => {
  it('should read as Solid colour', () => {
    renderField({ color: WHITE });

    expect(checked('Solid colour')).toBe('true');
    expect(checked('Image file')).toBe('false');
  });

  it('should switch to Image file and stay there, since an image fill carries a colour too', () => {
    const { onPatch } = renderField({ color: WHITE });

    fireEvent.click(screen.getByLabelText('Image file'));

    expect(onPatch).toHaveBeenCalledWith({ texture: undefined, textureMode: 1 });
    expect(checked('Image file')).toBe('true');
    expect(checked('Solid colour')).toBe('false');
  });
});

describe('when the source authors a texture', () => {
  it('should outrank an explicit pick, because the case is unambiguous', () => {
    renderField({
      color: WHITE,
      texture: { tex: { $case: 'texture', texture: { src: 'images/a.png' } } } as TextureUnion,
    });

    expect(checked('Image file')).toBe('true');

    fireEvent.click(screen.getByLabelText('Solid colour'));

    expect(checked('Image file')).toBe('true');
  });
});

describe('when nothing is authored', () => {
  it('should read as No fill', () => {
    renderField({});

    expect(checked('No fill')).toBe('true');
  });
});

describe('when the node is an Image with no texture yet', () => {
  it('should start on Image file with the Path field', () => {
    renderField({ textureMode: 1 });

    expect(checked('Image file')).toBe('true');
    expect(screen.getByText('Path')).toBeTruthy();
  });

  it('should read as Solid colour when it paints a colour, which is what it shows', () => {
    renderField({ color: WHITE, textureMode: 0 });

    expect(checked('Solid colour')).toBe('true');
  });

  it('should keep the texture mode on a switch to Solid colour, so it stays an Image', () => {
    const { onPatch } = renderField({ textureMode: 1 });

    fireEvent.click(screen.getByLabelText('Solid colour'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({ texture: undefined, color: WHITE });
  });

  it('should write a clear colour on a switch to No fill, which paints nothing and reads back as No fill', () => {
    const { onPatch } = renderField({ textureMode: 1 });

    fireEvent.click(screen.getByLabelText('No fill'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({ texture: undefined, color: CLEAR });
  });
});

describe('when an Image has No fill', () => {
  it('should read back as No fill', () => {
    renderField({ textureMode: 1, color: CLEAR });

    expect(checked('No fill')).toBe('true');
  });

  it('should read any other colour at 0% as Solid colour, keeping its hex and opacity', () => {
    renderField({ textureMode: 1, color: { r: 1, g: 60 / 255, b: 60 / 255, a: 0 } });

    expect(checked('Solid colour')).toBe('true');
    expect((screen.getAllByLabelText('Hex color')[0] as HTMLInputElement).value).toMatch(/FF3C3C/i);
    expect((screen.getAllByLabelText('Opacity percent')[0] as HTMLInputElement).value).toBe('0');
  });

  it('should start Solid colour from opaque white, not the clear colour', () => {
    const { onPatch } = renderField({ textureMode: 1, color: CLEAR });

    fireEvent.click(screen.getByLabelText('Solid colour'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({ texture: undefined, color: WHITE });
  });

  it('should drop the clear colour on a switch to Image file, so it reads back as Image file', () => {
    const { onPatch } = renderField({ textureMode: 1, color: CLEAR });

    fireEvent.click(screen.getByLabelText('Image file'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({
      texture: undefined,
      textureMode: 1,
      color: undefined,
    });
  });

  it('should tint an avatar opaque white rather than clear', () => {
    const { onPatch } = renderField({ textureMode: 1, color: CLEAR });

    fireEvent.click(screen.getByLabelText('Avatar'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({
      texture: { tex: { $case: 'avatarTexture', avatarTexture: { userId: '' } } },
      color: WHITE,
    });
  });
});

describe('when a textured Image has no texture mode in its source', () => {
  it('should add one on a switch to Solid colour, so it stays an Image', () => {
    const { onPatch } = renderField({ color: WHITE, texture: IMAGE_TEXTURE });

    fireEvent.click(screen.getByLabelText('Solid colour'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({
      texture: undefined,
      color: WHITE,
      textureMode: 1,
    });
  });

  it('should add one on a switch to No fill, so it stays an Image', () => {
    const { onPatch } = renderField({ color: WHITE, texture: IMAGE_TEXTURE });

    fireEvent.click(screen.getByLabelText('No fill'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({
      texture: undefined,
      color: CLEAR,
      textureMode: 1,
    });
  });
});

describe('when a Container has No fill', () => {
  it('should clear its colour, as before', () => {
    const { onPatch } = renderField({ color: WHITE });

    fireEvent.click(screen.getByLabelText('No fill'));

    expect(onPatch.mock.calls[0][0]).toStrictEqual({ texture: undefined, color: undefined });
  });

  it('should keep reading an authored clear colour as Solid colour', () => {
    renderField({ color: CLEAR });

    expect(checked('Solid colour')).toBe('true');
  });
});
