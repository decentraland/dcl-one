import { getStandardInput, setStandardInput } from './utils';

describe('InputModifier utils', () => {
  it('should read no blocked moves from an empty component', () => {
    expect(getStandardInput({})).toEqual({});
    expect(getStandardInput(undefined)).toEqual({});
  });

  it('should keep the other flags when one changes', () => {
    const value = { mode: { $case: 'standard' as const, standard: { disableJump: true } } };
    const next = setStandardInput(value, 'disableRun', true);
    expect(getStandardInput(next)).toEqual({ disableJump: true, disableRun: true });
  });

  it('should write the standard mode to a component that has none', () => {
    expect(setStandardInput({}, 'disableAll', true)).toEqual({
      mode: { $case: 'standard', standard: { disableAll: true } },
    });
  });
});
