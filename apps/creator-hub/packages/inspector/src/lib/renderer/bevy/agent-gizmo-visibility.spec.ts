import { isGizmoActive } from '../../../../agents/bevy/src/gizmo-visibility';

describe('isGizmoActive', () => {
  let state: Parameters<typeof isGizmoActive>[0];

  beforeEach(() => {
    state = { editingEnabled: true, hasTarget: true, modeDrawsHandles: true };
  });

  describe('when editing is on and a target is selected in a mode that draws handles', () => {
    it('should draw the gizmo', () => {
      expect(isGizmoActive(state)).toBe(true);
    });
  });

  describe('when editing is off (Interact on)', () => {
    beforeEach(() => {
      state.editingEnabled = false;
    });

    it('should not draw the gizmo even though a target is still selected', () => {
      expect(isGizmoActive(state)).toBe(false);
    });

    describe('and editing is turned back on', () => {
      beforeEach(() => {
        state.editingEnabled = true;
      });

      it('should draw the gizmo for the selection that is still held', () => {
        expect(isGizmoActive(state)).toBe(true);
      });
    });
  });

  describe('when nothing is selected', () => {
    beforeEach(() => {
      state.hasTarget = false;
    });

    it('should not draw the gizmo', () => {
      expect(isGizmoActive(state)).toBe(false);
    });
  });

  describe('when the mode draws no handles', () => {
    beforeEach(() => {
      state.modeDrawsHandles = false;
    });

    it('should not draw the gizmo', () => {
      expect(isGizmoActive(state)).toBe(false);
    });
  });
});
