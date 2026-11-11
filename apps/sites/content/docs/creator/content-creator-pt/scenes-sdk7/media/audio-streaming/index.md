> Reproduza streams de áudio ao vivo na sua scene.

# Streaming de Áudio

Você pode transmitir áudio a partir de uma URL. Isso é útil para tocar música diretamente de uma rádio da internet ou transmitir uma conferência para a sua Scene.


> [!NOTE]
> **💡 Dica**: Na [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), você pode usar um **Fluxo de áudio** [Smart Item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para uma maneira sem código de conseguir isso.


O áudio na fonte deve estar em um dos seguintes formatos: `.mp3`, `ogg`, ou `aac`. A fonte também deve ser um *https* URL (*http* URLs não são suportados), e a origem deve ter [políticas CORS (Cross Origin Resource Sharing)](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) que permitam acessá-la externamente. Se não for o caso, talvez você precise configurar um servidor para atuar como proxy e expor o stream de forma válida.


> [!WARNING]
> **📔 Nota**: Para tocar um som pré-gravado em sua Scene, veja [Sons](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/).


Para adicionar um fluxo de áudio à sua Scene, basta adicionar um `AudioStream` componente a uma Entity:

```ts
const streamEntity = engine.addEntity()

AudioStream.create(streamEntity, {
	url: 'https://icecast.ravepartyradio.org/ravepartyradio-192.mp3',
	playing: true,
	volume: 0.8,
})
```


> [!WARNING]
> **📔 Nota**: O som transmitido não é posicional; ele será ouvido em um volume constante em toda a sua Scene. Se um player sair da Scene, ele não ouvirá a transmissão de forma alguma.


Defina o volume do `AudioStream` componente alterando seu `volume` propriedade.

Alterne o `AudioStream` componente para ligado ou desligado definindo seu `playing` propriedade para *true* ou *false*.


> [!NOTE]
> **📔 Nota**: Nem todos os serviços de streaming permitem reproduzir o áudio fora do próprio site. A seguir estão alguns exemplos que funcionam no Decentraland:
> 
> ```ts
> DELTA = "https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e"
> GRAFFITI = "https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5"
> ISLA NEGRA = "https://radioislanegra.org/listen/up/basic.aac"
> ```
> 


## Estado da transmissão

Você pode monitorar o estado de um stream de áudio usando o `audioEventsSystem` (baseado em callback, recomendado) ou fazendo polling com `AudioStream.getAudioState()`.

### Usando audioEventsSystem (recomendado)

Registre um callback que é disparado apenas quando o estado do stream muda. Este é o mesmo sistema usado para `AudioSource` entities (veja [Detectar quando um som termina](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/#detect-when-a-sound-finishes)).

```ts
import { engine, AudioStream, MediaState } from '@dcl/sdk/ecs'
import { audioEventsSystem } from '@dcl/sdk/ecs'

export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	audioEventsSystem.registerAudioEventsEntity(entity, (event) => {
		console.log('Stream state: ', event.state)

		if (event.state === MediaState.MS_ERROR) {
			// Tentar reconexão
		}
	})
}
```

### Polling com getAudioState

Você também pode fazer polling do estado de um stream a cada frame usando `AudioStream.getAudioState()`. Isso retorna um `PBAudioEvent` object (ou `undefined` se nenhum estado tiver sido reportado ainda), com um `estado` campo e um `timestamp` campo. O `estado` campo é um valor de `MediaState` enum:

* `MS_NONE`
* `MS_LOADING`
* `MS_READY`
* `MS_PLAYING`
* `MS_PAUSED`
* `MS_BUFFERING`
* `MS_SEEKING`
* `MS_ERROR`

```ts
export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	let lastState: MediaState | undefined = undefined
	engine.addSystem(() => {
		const currentState = AudioStream.getAudioState(entity)?.state
		if (lastState !== currentState) {
			console.log('Stream state: ', currentState)
			lastState = currentState

			if (currentState == MediaState.MS_ERROR) {
				// Tentar reconexão
			}
		}
	})
}
```

## Áudio espacial

Por padrão, o áudio de um `AudioStream` componente é global, o que significa que ele será ouvido com um volume consistente em toda a sua cena. Se um jogador sair da cena, ele não ouvirá a transmissão de forma alguma.

Para tornar o áudio espacial, defina o `spatial` propriedade para *true*.

```ts
AudioStream.create(entity, {
	url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
	spatial: true,
})
```

O áudio agora será ouvido a partir da posição da entity que possui o `AudioStream` componente, e ficará mais alto à medida que o jogador se aproximar dele.

Controle o áudio espacial com as seguintes propriedades:

* `spatialMinDistance`: A distância mínima na qual o áudio se torna espacial. Se o jogador estiver mais perto, o áudio será ouvido no volume máximo. *0* por padrão.
* `spatialMaxDistance`: A distância máxima na qual o áudio é ouvido. Se o jogador estiver mais longe, o áudio será ouvido com volume 0. *60* por padrão

```ts
const audioStreamEntity = engine.addEntity();

Transform.create(audioStreamEntity, {
    position: Vector3.create(8, 0, 8),
});

AudioStream.create(audioStreamEntity, {
    url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
    volume: 1.0,
    spatial: true,
    spatialMinDistance: 5,
    spatialMaxDistance: 10
});
```


> [!WARNING]
> **📔 Nota**: Alguns formatos de áudio não suportam áudio espacial. Certifique-se de que o áudio do stream esteja codificado em *mp3*, *AAC-LC* ou *FLAC*.


## Análise de áudio

Você pode ler dados de amplitude e frequência em tempo real de uma `AudioStream` entity para acionar visuais reativos como equalizadores ou luzes pulsantes no ritmo. Veja [Análise de áudio](/docs/creator/content-creator-pt/scenes-sdk7/media/audio-analysis/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/media/audio-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
