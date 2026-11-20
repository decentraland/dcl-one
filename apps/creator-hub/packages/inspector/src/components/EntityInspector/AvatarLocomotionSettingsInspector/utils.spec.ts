import { setLocomotionSetting } from './utils';

describe('setLocomotionSetting', () => {
  it('should set a typed number', () => {
    expect(setLocomotionSetting({ jumpHeight: 1 }, 'runSpeed', '12.5')).toEqual({
      jumpHeight: 1,
      runSpeed: 12.5,
    });
  });

  it('should unset an emptied field so the default applies', () => {
    expect(setLocomotionSetting({ runSpeed: 12, jumpHeight: 1 }, 'runSpeed', ' ')).toEqual({
      jumpHeight: 1,
    });
  });

  it('should clamp a negative number to 0, as explorers do', () => {
    expect(setLocomotionSetting({}, 'walkSpeed', '-3')).toEqual({ walkSpeed: 0 });
  });

  it('should ignore text that is not a number', () => {
    expect(setLocomotionSetting({ walkSpeed: 2 }, 'walkSpeed', 'abc')).toBeNull();
  });
});
