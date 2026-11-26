import { useReducer, useRef } from 'react';
import { flushSync } from 'react-dom';
import { act, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { useOverlayPosition } from '@ui/atoms/useOverlayPosition';

// ui3's own tests run on React 19. The loop below needs React 18's update queue, which is what the app
// runs, so the regression test lives here.

const point = (x: number, y: number) => DOMRect.fromRect({ x, y, width: 0, height: 0 });

describe('useOverlayPosition under React 18', () => {
  it('does not render in a loop when a measure from outside React is still pending', () => {
    const at = { x: 100, y: 120 };
    let bump = () => {};
    function Menu() {
      bump = useReducer((n: number) => n + 1, 0)[1];
      const surface = useRef<HTMLDivElement>(null);
      const position = useOverlayPosition(
        true,
        { getBoundingClientRect: () => point(at.x, at.y) },
        surface,
      );
      return (
        <div
          ref={surface}
          data-testid="menu"
          style={{ left: position.left, top: position.top }}
        />
      );
    }
    render(<Menu />);

    // The menu moves, so the resize listener queues a normal-priority update that has not run yet.
    at.y = 150;
    window.dispatchEvent(new Event('resize'));
    // A synchronous render measures again before it does (deleting a chat from the AI panel's menu).
    at.y = 128;
    expect(() => flushSync(() => bump())).not.toThrow();
    act(() => {});

    expect(screen.getByTestId('menu').style.top).toBe('128px');
  });
});
