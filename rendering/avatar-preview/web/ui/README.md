# Preview controls assets

The icons, cursors and font the Unity avatar preview renderer draws its controls with, copied
unchanged from `decentraland/unity-explorer` (Apache-2.0) at `0949e3ca5d`,
`avatar-preview-renderer/Assets/UI/`:

- `Sprites/BodyIcon.png`, `UpperBodyIcon.png` (the view switcher), `PlayArrow.png`, `Stop.png`,
  `SoundOn.png`, `SoundOff.png` (the emote controls), `Spinner.png` (the loader);
- `Sprites/Configurator/cursor_rotate.png`, `cursor_pan.png`, `cursor_interactive.png`;
- `Font/Inter-UI-Bold.otf` (Inter, SIL Open Font License 1.1).

`web/index.html` lays them out as the renderer's `UI/Preview/PreviewUI.uxml` and
`PreviewStyle.uss` do, in its 700x700 reference space scaled to the panel.
