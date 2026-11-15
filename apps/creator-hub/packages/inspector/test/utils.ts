import { vi } from 'vitest';

// A mock that the code calls with `new` needs a `function`, not an arrow function.
const xhrMock = vi.fn().mockImplementation(function () {
  return {
    open: vi.fn(),
    send: vi.fn(),
    setRequestHeader: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    abort: vi.fn(),
  };
});

export function mockXMLHttpRequest() {
  const global = globalThis as any;
  if (global.XMLHttpRequest !== xhrMock) {
    const prev = global.XMLHttpRequest;
    global.XMLHttpRequest = xhrMock;
    return () => {
      global.XMLHttpRequest = prev;
    };
  }
  return () => {};
}
