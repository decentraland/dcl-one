> Lee datos de amplitud y frecuencia en tiempo real del audio que se reproduce en tu escena para impulsar visuales reactivos.

# Análisis de audio

El `AudioAnalysis` componente lee datos en tiempo real de una fuente de audio que se reproduce en tu escena, así que puedes controlar los visuales a partir de la música. En cada frame, reporta un valor general de **amplitud** y **8 bandas de frecuencia** (de baja a alta) que puedes usar para escalar, colorear o animar entities.

Usos comunes:

* Cubos que rebotan con el bajo.
* Luces que laten al ritmo.
* Visualizadores de barras al estilo ecualizador.
* Materiales reactivos u objetos escalados que acompañan la música.

`AudioAnalysis` es un flujo de solo lectura — tu escena solo consume los valores, el Runtime los completa. Funciona en entities que reproducen audio a través de un [AudioSource](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/), un [AudioStream](/docs/creator/content-creator-es/scenes-sdk7/media/audio-streaming/), o el audio de un [VideoPlayer](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/).


> [!WARNING]
> **📔 Nota**: `AudioAnalysis` actualmente solo es compatible con la app de escritorio oficial de Decentraland. En otros clients los valores del componente nunca se completan, así que asegúrate de que tu escena siga funcionando si los datos nunca llegan.


## Ejemplo mínimo

La siguiente escena reproduce un sonido, adjunta `AudioAnalysis` a la misma entity y escala un cubo en cada frame usando la amplitud del audio.

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
  // Reproduce un sonido en una entity
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });

  // Adjunta AudioAnalysis a la misma entity para empezar a recibir datos de análisis
  AudioAnalysis.createAudioAnalysis(audioEntity);

  // Un objeto de vista reutilizable en el que el componente escribirá en cada frame
  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // Un cubo que late con la amplitud del audio
  const cube = engine.addEntity();
  MeshRenderer.setBox(cube);
  Transform.create(cube, { position: Vector3.create(8, 1, 8) });

  // En cada frame, lee los últimos valores de análisis y escala el cubo
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
    const s = 1 + analysis.amplitude * 10;
    Transform.getMutable(cube).scale = Vector3.create(s, s, s);
  });
}
```

Las llamadas clave son:

1. `AudioAnalysis.createAudioAnalysis(entity)` — adjunta el componente a la entity que posee el `AudioSource`, `AudioStream`, o `VideoPlayer`. Usa por defecto el modo logarítmico (ver [Modos](#modes)).
2. `AudioAnalysis.readIntoView(entity, analysis)` — copia los últimos `amplitud` y 8 `bandas` valores en un objeto de vista que proporcionas.


> [!WARNING]
> **📔 Nota**: Preasigna siempre el `bandas` array con **8 elementos** antes de llamar a `readIntoView`. El componente no redimensiona el array por ti.


## Leer los datos en cada frame

`AudioAnalysis` está diseñado para leerse una vez por frame desde un System. El patrón recomendado es:

1. Asigna un único `AudioAnalysisView` objeto una sola vez.
2. En un System, llama a `readIntoView` para actualizarlo.
3. Usa los valores directamente, o comparte la vista entre varios Systems.

```ts
const analysis: AudioAnalysisView = {
  amplitude: 0,
  bands: new Array<number>(8),
};

engine.addSystem(() => {
  AudioAnalysis.readIntoView(audioEntity, analysis);
  // analysis.amplitude y analysis.bands[0..7] ahora contienen los últimos valores
});
```

Si la entity analizada aún no tiene un `AudioAnalysis` componente (por ejemplo, se crea más tarde o se elimina dinámicamente), usa `tryReadIntoView` en su lugar. Devuelve `false` cuando falta el componente, y `true` cuando se escriben los valores.

```ts
engine.addSystem(() => {
  if (!AudioAnalysis.tryReadIntoView(audioEntity, analysis)) return;
  // es seguro usar analysis aquí
});
```


> [!NOTE]
> **💡 Consejo**: Es más barato llamar a `readIntoView` una vez y dejar que varios Systems compartan el mismo objeto de vista que leer el componente repetidamente.


## Reacciona a bandas de frecuencia específicas

Las 8 bandas cubren el espectro audible de bajos a altos. `bands[0]` es la más baja (bajo) y `bands[7]` es la más alta (agudos). Puedes controlar diferentes entities desde distintas bandas para construir un ecualizador clásico.

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

// Componente personalizado para marcar cada barra y recordar qué banda sigue
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

  // Crea 8 barras, una por banda
  for (let i = 0; i < 8; i++) {
    const bar = engine.addEntity();
    VisualBar.create(bar, { index: i });
    Transform.create(bar, { position: Vector3.create(4 + i, 0, 8) });
    MeshRenderer.setBox(bar);
    Material.setPbrMaterial(bar, { albedoColor: Color4.Yellow() });
  }

  // Lee una vez por frame
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
  });

  // Escala cada barra con su banda
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

Este es el mismo patrón usado en el [scene de ejemplo de visualización de audio](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization), recortado a lo esencial.

## Modos

`AudioAnalysis` admite dos modos de análisis, establecidos cuando creas el componente:

* `PBAudioAnalysisMode.MODE_LOGARITHMIC` (predeterminado): los valores se escalan con una curva logarítmica, que se siente más cercana a cómo la audición humana percibe los cambios de volumen. Mejor para la reactividad visual.
* `PBAudioAnalysisMode.MODE_RAW`: la amplitud sin procesar y los valores de bandas FFT. Usa esto si quieres aplicar tu propia escala.

```ts
import { AudioAnalysis, PBAudioAnalysisMode } from "@dcl/sdk/ecs";

// Usa valores sin procesar
AudioAnalysis.createAudioAnalysis(audioEntity, PBAudioAnalysisMode.MODE_RAW);
```

### Ajustar el modo logarítmico

En modo logarítmico puedes pasar dos multiplicadores de ganancia opcionales:

* `amplitudeGain` — multiplicador aplicado a la amplitud general. El valor predeterminado es `5`.
* `bandsGain` — multiplicador aplicado a las 8 bandas. El valor predeterminado es `0.05`.

Una ganancia más alta hace que los valores sean más reactivos; una ganancia más baja los hace más sutiles.

```ts
// Reacción de amplitud más fuerte, reacción de banda más suave
AudioAnalysis.createAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_LOGARITHMIC,
  10, // amplitudeGain
  0.03 // bandsGain
);
```


> [!WARNING]
> **📔 Nota**: `amplitudeGain` y `bandsGain` solo se aplican en modo logarítmico. En modo raw se ignoran.


## Reemplazar un componente existente

`createAudioAnalysis` falla si la entity ya tiene un `AudioAnalysis` componente. Para cambiar de modo o de ganancia en Runtime, usa `createOrReplaceAudioAnalysis`, que tiene la misma firma.

```ts
// Cambia a modo raw a mitad de la escena
AudioAnalysis.createOrReplaceAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_RAW
);
```

## Referencia del componente

### `AudioAnalysis.createAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

Adjunta un `AudioAnalysis` componente a `entity`. Falla si ya hay un componente presente.

* `entity` (`Entity`): la entity que posee el `AudioSource`, `AudioStream`, o `VideoPlayer` que quieres analizar.
* `mode` (`PBAudioAnalysisMode`, opcional): `MODE_LOGARITHMIC` (predeterminado) o `MODE_RAW`.
* `amplitudeGain` (`number`, opcional): multiplicador de amplitud en modo logarítmico. Default `5`.
* `bandsGain` (`number`, opcional): multiplicador de bandas en modo logarítmico. Default `0.05`.

### `AudioAnalysis.createOrReplaceAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

Igual que arriba, pero reemplaza un `AudioAnalysis` componente en la entity en lugar de fallar.

### `AudioAnalysis.readIntoView(entity, out)`

Lee los últimos valores en `out`. Lanza un error si `entity` no tiene un `AudioAnalysis` componente.

* `entity` (`Entity`): la entity con el componente.
* `out` (`AudioAnalysisView`): un objeto de vista que proporcionas. Debe tener `bandas` preasignado con 8 números.

### `AudioAnalysis.tryReadIntoView(entity, out): boolean`

Igual que `readIntoView`, pero devuelve `false` si la entity no tiene ningún `AudioAnalysis` componente en lugar de lanzar un error. Devuelve `true` cuando se escriben los valores.

### `AudioAnalysisView`

Un objeto simple que asignas para recibir los datos de análisis:

```ts
type AudioAnalysisView = {
  amplitude: number; // fuerza general de la señal
  bands: number[]; // 8 bandas de frecuencia, de baja (0) a alta (7)
};
```

## Notas y limitaciones

* **8 bandas, fijas.** El número de bandas de frecuencia está fijo en 8. No hay ninguna API para solicitar más o menos bandas.
* **Una fuente de audio por análisis.** Cada `AudioAnalysis` componente analiza el audio de la entity a la que está adjunto. Para analizar varias fuentes, adjunta `AudioAnalysis` a cada una.
* **Las bandas sonoras de video también se analizan.** `AudioAnalysis` lee el audio de un [`VideoPlayer`](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/) de la misma manera que lee un `AudioSource`, tanto para archivos de video progresivos (como `.mp4`) y streams HLS en vivo (`.m3u8`). Adjunta `AudioAnalysis` a la misma entity que contiene el `VideoPlayer`.
* **El audio en pausa no reporta actualizaciones.** Cuando el audio subyacente se detiene o se pausa, los valores del componente dejan de cambiar. Tus últimos valores leídos permanecen en tu objeto de vista hasta que el audio vuelva a reproducirse.
* **Barato, pero no gratis.** El análisis por frame está diseñado para ser económico (menos de un milisegundo por fuente en desktop). Evita adjuntar `AudioAnalysis` a demasiadas fuentes a la vez si no necesitas los datos. Elimina el componente cuando un visualizador no sea visible.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/media/audio-analysis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
