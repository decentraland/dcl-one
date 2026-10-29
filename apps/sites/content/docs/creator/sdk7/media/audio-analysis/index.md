> Read real-time amplitude and frequency data from audio playing in your scene to drive reactive visuals.

# Audio Analysis

The `AudioAnalysis` component reads real-time data from an audio source playing in your scene, so you can drive visuals from the music. Each frame, it reports an overall **amplitude** value and **8 frequency bands** (low to high) you can use to scale, color, or animate entities.

Common uses:

- Cubes that bounce on the bass.
- Lights that pulse with the beat.
- Equalizer-style bar visualizers.
- Reactive materials or scaled props that match the music.

`AudioAnalysis` is a read-only feed — your scene only consumes the values, the runtime fills them in. It works on entities that play audio through an [AudioSource](/docs/creator/sdk7/3d-essentials/sounds/), an [AudioStream](/docs/creator/sdk7/media/audio-streaming/), or the audio of a [VideoPlayer](/docs/creator/sdk7/media/video-playing/).


> [!WARNING]
> **📔 Note**: `AudioAnalysis` is currently only supported by the official Decentraland desktop app. On other clients the component's values are never filled in, so make sure your scene still works if the data never arrives.


## Minimal example

The following scene plays a sound, attaches `AudioAnalysis` to the same entity, and scales a cube on every frame using the audio's amplitude.

```ts
import {
  engine,
  Transform,
  MeshRenderer,
  AudioSource,
  AudioAnalysis,
  AudioAnalysisView,
} from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export function main() {
  // Play a sound on an entity
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });

  // Attach AudioAnalysis to the same entity to start receiving analysis data
  AudioAnalysis.createAudioAnalysis(audioEntity);

  // A reusable view object that the component will write into each frame
  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // A cube that pulses with the audio amplitude
  const cube = engine.addEntity();
  MeshRenderer.setBox(cube);
  Transform.create(cube, { position: Vector3.create(8, 1, 8) });

  // Each frame, read the latest analysis values and scale the cube
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
    const s = 1 + analysis.amplitude * 10;
    Transform.getMutable(cube).scale = Vector3.create(s, s, s);
  });
}
```

The key calls are:

1. `AudioAnalysis.createAudioAnalysis(entity)` — attaches the component to the entity that owns the `AudioSource`, `AudioStream`, or `VideoPlayer`. Defaults to logarithmic mode (see [Modes](#modes)).
2. `AudioAnalysis.readIntoView(entity, analysis)` — copies the latest `amplitude` and 8 `bands` values into a view object you provide.


> [!WARNING]
> **📔 Note**: Always pre-allocate the `bands` array with **8 elements** before you call `readIntoView`. The component does not resize the array for you.


## Reading the data each frame

`AudioAnalysis` is designed to be read once per frame from a system. The recommended pattern is:

1. Allocate a single `AudioAnalysisView` object once.
2. In a system, call `readIntoView` to refresh it.
3. Use the values directly, or share the view between several systems.

```ts
const analysis: AudioAnalysisView = {
  amplitude: 0,
  bands: new Array<number>(8),
};

engine.addSystem(() => {
  AudioAnalysis.readIntoView(audioEntity, analysis);
  // analysis.amplitude and analysis.bands[0..7] now hold the latest values
});
```

If the analyzed entity might not yet have an `AudioAnalysis` component (for example, it's created later or removed dynamically), use `tryReadIntoView` instead. It returns `false` when the component is missing, and `true` when values were written.

```ts
engine.addSystem(() => {
  if (!AudioAnalysis.tryReadIntoView(audioEntity, analysis)) return;
  // safe to use analysis here
});
```


> [!NOTE]
> **💡 Tip**: It's cheaper to call `readIntoView` once and let multiple systems share the same view object than to read the component repeatedly.


## React to specific frequency bands

The 8 bands cover the audible spectrum from low to high. `bands[0]` is the lowest (bass) and `bands[7]` is the highest (treble). You can drive different entities from different bands to build a classic equalizer.

```ts
import {
  engine,
  Transform,
  MeshRenderer,
  Material,
  AudioSource,
  AudioAnalysis,
  AudioAnalysisView,
  Schemas,
} from "@dcl/sdk/ecs";
import { Color4, Vector3 } from "@dcl/sdk/math";

// Custom component to mark each bar and remember which band it tracks
const VisualBar = engine.defineComponent("visual-bar", {
  index: Schemas.Number,
});

const BARS_HEIGHT = 12;

export function main() {
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });
  AudioAnalysis.createAudioAnalysis(audioEntity);

  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // Create 8 bars, one per band
  for (let i = 0; i < 8; i++) {
    const bar = engine.addEntity();
    VisualBar.create(bar, { index: i });
    Transform.create(bar, { position: Vector3.create(4 + i, 0, 8) });
    MeshRenderer.setBox(bar);
    Material.setPbrMaterial(bar, { albedoColor: Color4.Yellow() });
  }

  // Read once per frame
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
  });

  // Scale each bar with its band
  engine.addSystem(() => {
    for (const [entity] of engine.getEntitiesWith(VisualBar, Transform)) {
      const index = VisualBar.get(entity).index;
      const transform = Transform.getMutable(entity);
      transform.scale = Vector3.create(
        1,
        analysis.bands[index] * BARS_HEIGHT,
        1
      );
    }
  });
}
```

This is the same pattern used in the [audio-visualization example scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization), trimmed to the essentials.

## Modes

`AudioAnalysis` supports two analysis modes, set when you create the component:

- `PBAudioAnalysisMode.MODE_LOGARITHMIC` (default): values are scaled with a logarithmic curve, which feels closer to how human hearing perceives volume changes. Best for visual reactivity.
- `PBAudioAnalysisMode.MODE_RAW`: the unprocessed amplitude and FFT band values. Use this if you want to apply your own scaling.

```ts
import { AudioAnalysis, PBAudioAnalysisMode } from "@dcl/sdk/ecs";

// Use raw values
AudioAnalysis.createAudioAnalysis(audioEntity, PBAudioAnalysisMode.MODE_RAW);
```

### Tuning the logarithmic mode

In logarithmic mode you can pass two optional gain multipliers:

- `amplitudeGain` — multiplier applied to the overall amplitude. Defaults to `5`.
- `bandsGain` — multiplier applied to all 8 bands. Defaults to `0.05`.

Higher gains make the values more reactive; lower gains make them subtler.

```ts
// Stronger amplitude reaction, gentler band reaction
AudioAnalysis.createAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_LOGARITHMIC,
  10, // amplitudeGain
  0.03 // bandsGain
);
```


> [!WARNING]
> **📔 Note**: `amplitudeGain` and `bandsGain` only apply in logarithmic mode. In raw mode they're ignored.


## Replace an existing component

`createAudioAnalysis` fails if the entity already has an `AudioAnalysis` component. To swap modes or gains at runtime, use `createOrReplaceAudioAnalysis`, which has the same signature.

```ts
// Switch to raw mode mid-scene
AudioAnalysis.createOrReplaceAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_RAW
);
```

## Component reference

### `AudioAnalysis.createAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

Attaches an `AudioAnalysis` component to `entity`. Fails if a component is already present.

- `entity` (`Entity`): the entity that owns the `AudioSource`, `AudioStream`, or `VideoPlayer` you want to analyze.
- `mode` (`PBAudioAnalysisMode`, optional): `MODE_LOGARITHMIC` (default) or `MODE_RAW`.
- `amplitudeGain` (`number`, optional): logarithmic-mode amplitude multiplier. Default `5`.
- `bandsGain` (`number`, optional): logarithmic-mode bands multiplier. Default `0.05`.

### `AudioAnalysis.createOrReplaceAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

Same as above, but replaces an existing `AudioAnalysis` component on the entity instead of failing.

### `AudioAnalysis.readIntoView(entity, out)`

Reads the latest values into `out`. Throws if `entity` does not have an `AudioAnalysis` component.

- `entity` (`Entity`): the entity with the component.
- `out` (`AudioAnalysisView`): a view object you provide. Must have `bands` pre-allocated with 8 numbers.

### `AudioAnalysis.tryReadIntoView(entity, out): boolean`

Same as `readIntoView`, but returns `false` if the entity has no `AudioAnalysis` component instead of throwing. Returns `true` when values are written.

### `AudioAnalysisView`

A plain object you allocate to receive the analysis data:

```ts
type AudioAnalysisView = {
  amplitude: number; // overall signal strength
  bands: number[]; // 8 frequency bands, low (0) to high (7)
};
```

## Notes and limitations

- **8 bands, fixed.** The number of frequency bands is fixed at 8. There is no API to request more or fewer bands.
- **One audio source per analysis.** Each `AudioAnalysis` component analyzes the audio from the entity it is attached to. To analyze several sources, attach `AudioAnalysis` to each one.
- **Live video streams are not analyzed.** There is a known issue where `AudioAnalysis` receives no data when the `VideoPlayer` source is a non-progressive stream, like an `.m3u8` HLS URL: the stream's audio plays, but the analysis values stay at zero. Video files (like `.mp4`) are analyzed correctly.
- **Paused audio reports no updates.** When the underlying audio is stopped or paused, the component values stop changing. Your last-read values remain in your view object until the audio plays again.
- **Cheap, but not free.** Per-frame analysis is designed to be inexpensive (sub-millisecond per source on desktop). Avoid attaching `AudioAnalysis` to many sources at once if you don't need the data. Remove the component when a visualizer isn't visible.
