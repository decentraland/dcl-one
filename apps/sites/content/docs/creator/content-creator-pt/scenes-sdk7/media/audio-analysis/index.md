> Leia dados de amplitude e frequência em tempo real do áudio reproduzido na sua scene para acionar visuais reativos.

# Análise de Áudio

O `AudioAnalysis` component lê dados em tempo real de uma fonte de áudio tocando na sua scene, para que você possa dirigir os visuais pela música. A cada frame, ele reporta uma amplitude geral **amplitude** valor e **8 bandas de frequência** (de baixo para alto) que você pode usar para redimensionar, colorir ou animar entities.

Usos comuns:

* Cubos que pulsam com o baixo.
* Luzes que pulsam com a batida.
* Visualizadores de barras no estilo equalizador.
* Materials reativos ou props redimensionados que combinam com a música.

`AudioAnalysis` é um feed somente de leitura — sua scene apenas consome os valores, o runtime os preenche. Funciona em entities que reproduzem áudio por meio de um [AudioSource](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/), um [AudioStream](/docs/creator/content-creator-pt/scenes-sdk7/media/audio-streaming/), ou o áudio de um [VideoPlayer](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/).


> [!WARNING]
> **📔 Nota**: `AudioAnalysis` é atualmente suportado apenas pelo app desktop oficial da Decentraland. Em outros clients, os valores do component nunca são preenchidos, então certifique-se de que sua scene ainda funcione se os dados nunca chegarem.


## Exemplo mínimo

A scene a seguir reproduz um som, anexa `AudioAnalysis` à mesma entity e redimensiona um cubo a cada frame usando a amplitude do áudio.

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
  // Toca um som em uma entity
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });

  // Anexa AudioAnalysis à mesma entity para começar a receber dados de análise
  AudioAnalysis.createAudioAnalysis(audioEntity);

  // Um objeto de view reutilizável no qual o component escreverá a cada frame
  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // Um cubo que pulsa com a amplitude do áudio
  const cube = engine.addEntity();
  MeshRenderer.setBox(cube);
  Transform.create(cube, { position: Vector3.create(8, 1, 8) });

  // A cada frame, leia os valores mais recentes da análise e redimensione o cubo
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
    const s = 1 + analysis.amplitude * 10;
    Transform.getMutable(cube).scale = Vector3.create(s, s, s);
  });
}
```

As chamadas principais são:

1. `AudioAnalysis.createAudioAnalysis(entity)` — anexa o component à entity que possui o `AudioSource`, `AudioStream`, ou `VideoPlayer`. O padrão é o modo logarítmico (veja [Modos](#modes)).
2. `AudioAnalysis.readIntoView(entity, analysis)` — copia o mais recente `amplitude` e 8 `bands` valores para um objeto de view que você fornece.


> [!WARNING]
> **📔 Nota**: Sempre pré-aloque o `bands` array com **8 elementos** antes de chamar `readIntoView`. O component não redimensiona o array para você.


## Lendo os dados a cada frame

`AudioAnalysis` foi projetado para ser lido uma vez por frame a partir de um system. O padrão recomendado é:

1. Alocar um único `AudioAnalysisView` objeto uma vez.
2. Em um system, chame `readIntoView` para atualizá-lo.
3. Use os valores diretamente, ou compartilhe a view entre vários systems.

```ts
const analysis: AudioAnalysisView = {
  amplitude: 0,
  bands: new Array<number>(8),
};

engine.addSystem(() => {
  AudioAnalysis.readIntoView(audioEntity, analysis);
  // analysis.amplitude e analysis.bands[0..7] agora guardam os valores mais recentes
});
```

Se a entity analisada ainda não tiver um `AudioAnalysis` component (por exemplo, ele é criado mais tarde ou removido dinamicamente), use `tryReadIntoView` em vez disso. Ele retorna `false` quando o component está ausente, e `true` quando os valores foram escritos.

```ts
engine.addSystem(() => {
  if (!AudioAnalysis.tryReadIntoView(audioEntity, analysis)) return;
  // seguro usar analysis aqui
});
```


> [!NOTE]
> **💡 Dica**: É mais barato chamar `readIntoView` uma vez e deixar vários systems compartilharem o mesmo objeto de view do que ler o component repetidamente.


## Reaja a bandas de frequência específicas

As 8 bandas cobrem o espectro audível do baixo ao alto. `bands[0]` é a mais baixa (baixo) e `bands[7]` é a mais alta (agudo). Você pode dirigir different entities a partir de bands diferentes para construir um equalizador clássico.

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

// Component personalizado para marcar cada barra e lembrar qual band ela acompanha
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

  // Crie 8 barras, uma por band
  for (let i = 0; i < 8; i++) {
    const bar = engine.addEntity();
    VisualBar.create(bar, { index: i });
    Transform.create(bar, { position: Vector3.create(4 + i, 0, 8) });
    MeshRenderer.setBox(bar);
    Material.setPbrMaterial(bar, { albedoColor: Color4.Yellow() });
  }

  // Leia uma vez por frame
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
  });

  // Redimensione cada barra com sua band
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

Este é o mesmo padrão usado na [scene de exemplo de visualização de áudio](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization), reduzido ao essencial.

## Modos

`AudioAnalysis` suporta dois modos de análise, definidos quando você cria o component:

* `PBAudioAnalysisMode.MODE_LOGARITHMIC` (padrão): os valores são escalados com uma curva logarítmica, o que se aproxima mais de como a audição humana percebe mudanças de volume. Melhor para reatividade visual.
* `PBAudioAnalysisMode.MODE_RAW`: a amplitude não processada e os valores das bandas FFT. Use isto se quiser aplicar sua própria escala.

```ts
import { AudioAnalysis, PBAudioAnalysisMode } from "@dcl/sdk/ecs";

// Use valores raw
AudioAnalysis.createAudioAnalysis(audioEntity, PBAudioAnalysisMode.MODE_RAW);
```

### Ajustando o modo logarítmico

No modo logarítmico, você pode passar dois multiplicadores de ganho opcionais:

* `amplitudeGain` — multiplicador aplicado à amplitude geral. O padrão é `5`.
* `bandsGain` — multiplicador aplicado a todas as 8 bandas. O padrão é `0.05`.

Ganhos mais altos tornam os valores mais reativos; ganhos mais baixos os tornam mais sutis.

```ts
// Reação mais forte da amplitude, reação mais suave das bandas
AudioAnalysis.createAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_LOGARITHMIC,
  10, // amplitudeGain
  0.03 // bandsGain
);
```


> [!WARNING]
> **📔 Nota**: `amplitudeGain` e `bandsGain` só se aplicam no modo logarítmico. No modo raw, são ignorados.


## Substituir um component existente

`createAudioAnalysis` falha se a entity já tiver um `AudioAnalysis` component. Para alternar modos ou ganhos em runtime, use `createOrReplaceAudioAnalysis`, que tem a mesma assinatura.

```ts
// Mude para o modo raw no meio da scene
AudioAnalysis.createOrReplaceAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_RAW
);
```

## Referência ao component

### `AudioAnalysis.createAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

Anexa um `AudioAnalysis` component a `entity`. Falha se já houver um component.

* `entity` (`Entity`): a entity que possui o `AudioSource`, `AudioStream`, ou `VideoPlayer` que você quer analisar.
* `mode` (`PBAudioAnalysisMode`, opcional): `MODE_LOGARITHMIC` (padrão) ou `MODE_RAW`.
* `amplitudeGain` (`number`, opcional): multiplicador da amplitude no modo logarítmico. Padrão `5`.
* `bandsGain` (`number`, opcional): multiplicador das bandas no modo logarítmico. Padrão `0.05`.

### `AudioAnalysis.createOrReplaceAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

Igual ao acima, mas substitui um `AudioAnalysis` component existente na entity em vez de falhar.

### `AudioAnalysis.readIntoView(entity, out)`

Lê os valores mais recentes para `out`. Lança erro se `entity` não tiver um `AudioAnalysis` component.

* `entity` (`Entity`): a entity com o component.
* `out` (`AudioAnalysisView`): um objeto de view que você fornece. Deve ter `bands` pré-alocado com 8 números.

### `AudioAnalysis.tryReadIntoView(entity, out): boolean`

Igual a `readIntoView`, mas retorna `false` se a entity não tiver um `AudioAnalysis` component em vez de lançar erro. Retorna `true` quando os valores são escritos.

### `AudioAnalysisView`

Um objeto simples que você aloca para receber os dados de análise:

```ts
type AudioAnalysisView = {
  amplitude: number; // força geral do sinal
  bands: number[]; // 8 bandas de frequência, da baixa (0) à alta (7)
};
```

## Notas e limitações

* **8 bandas, fixas.** O número de bandas de frequência é fixo em 8. Não há API para solicitar mais ou menos bandas.
* **Uma fonte de áudio por análise.** Cada `AudioAnalysis` component analisa o áudio da entity à qual ele está anexado. Para analisar várias fontes, anexe `AudioAnalysis` a cada uma.
* **As trilhas sonoras de Video também são analisadas.** `AudioAnalysis` lê o áudio de um [`VideoPlayer`](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/) da mesma forma que lê um `AudioSource`, tanto para arquivos de vídeo progressivos (como `.mp4`) e streams HLS ao vivo (`.m3u8`). Anexe `AudioAnalysis` à mesma entity que contém o `VideoPlayer`.
* **Áudio pausado não reporta atualizações.** Quando o áudio subjacente é parado ou pausado, os valores do component deixam de mudar. Os últimos valores lidos permanecem no seu objeto de view até o áudio tocar novamente.
* **Barato, mas não gratuito.** A análise por frame foi projetada para ser econômica (submilissegundo por fonte no desktop). Evite anexar `AudioAnalysis` a muitas fontes ao mesmo tempo se você não precisar dos dados. Remova o component quando um visualizer não estiver visível.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/media/audio-analysis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
