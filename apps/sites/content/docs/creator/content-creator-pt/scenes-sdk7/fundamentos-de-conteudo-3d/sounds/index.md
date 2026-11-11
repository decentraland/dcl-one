> Saiba como adicionar sons à sua scene.

# Sons

O som é uma ótima maneira de fornecer feedback às ações e eventos do jogador; sons de fundo também podem dar mais contexto à sua Scene e melhorar a imersão do jogador nela.


> [!WARNING]
> **📔 Nota**: Tenha em mente que os sons só são ouvidos pelos jogadores que estiverem dentro dos parcels que compõem a Scene onde o som foi gerado, mesmo que estivessem dentro do alcance de audição. Os jogadores também podem optar por desativar todos os sons nas configurações.


Os formatos de som suportados variam conforme o navegador, mas é recomendado usar *.mp3*.

*.wav*  arquivos também são suportados, mas geralmente não são recomendados, pois são significativamente mais pesados.

## Reproduzir sons

A maneira mais fácil de reproduzir um som é adicionar um **AudioSource** component visualmente no [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/) e configurá-lo para **Começar a reproduzir** e **Loop**. Veja [Add Components](/docs/creator/content-creator-pt/scene-editor/construir/components/#add-components).

![](/docs/_remote/341e53c596dc617859805f3b.webp)

Você também pode acionar a reprodução de um som de forma sem código via **Ações**, veja [Transforme qualquer item em smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).

Para reproduzir um som via código, use a `AudioSource.playSound` função.

```ts
// Criar entity
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// Reproduzir som
AudioSource.playSound(sourceEntity, 'assets/sounds/sound-effect.mp3')
```


> [!WARNING]
> **📔 Nota**: Toda entity com um `AudioSource` também precisa de um `Transform`, inclusive sons globais. Sem um, nada é reproduzido e nenhum erro é registrado, o que torna esse problema difícil de identificar. Itens colocados no Scene Editor sempre têm um `Transform`Transform


O arquivo de som deve estar dentro da pasta do projeto. No exemplo acima, o arquivo de áudio está localizado em uma pasta `assets/sounds` , que está localizada na raiz da pasta do projeto da scene.


> [!WARNING]
> **📔 Nota**: O `AudioSource` component deve ser importado via
> 
> > `import { AudioSource } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar facilmente com isso.


O `AudioSource.playSound()` função recebe os seguintes argumentos:

* `entity`: Em qual entity aplicar o som. O som será ouvido a partir da posição dessa entity, o que significa que ele fica mais alto conforme o player se aproxima.
* `src`: O local do arquivo de som dentro do seu projeto.


> [!NOTE]
> **💡 Dica**: Para maior clareza, recomendamos manter seus arquivos de som separados em uma `assets/sounds` pasta dentro da sua scene.


* `resetCursor`: *(opcional)* Se true, o som sempre começa do início. Caso contrário, ele continua a partir da posição atual do cursor. Útil para pausar e retomar.

`playSound()` cria o `AudioSource` component se a entity ainda não tiver um. Se ela já tiver um, `playSound()` mantém os outros valores, como `volume`, `pitch`, `loop` e `global`, então você pode defini-los uma vez e depois chamar `playSound()` toda vez que quiser reproduzir o som. Chamá-la novamente com o mesmo arquivo reproduz o som de novo. Ela sempre retorna `true`.

Outra forma de reproduzir sons é criar manualmente um `AudioSource` component em uma entity. Use essa abordagem para ter mais controle sobre o som, por exemplo para fazê-lo entrar em loop ou definir o volume.

```ts
// Criar entity
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// Criar component AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	loop: true,
	playing: true,
})
```

As seguintes propriedades podem ser definidas:

* `audioClipUrl`: O local do arquivo de som dentro do seu projeto.
* `playing`: Se true, o som começa a tocar. Você pode criar um som com `playing` definido como false e depois defini-lo como true em um momento posterior.
* `volume`: *(opcional)* O volume do arquivo de som. 1 por padrão, que é o volume máximo.
* `pitch`: *(opcional)* Modifique o pitch de um som. 1 é o padrão; use um valor menor para um som mais grave e maior para um som mais agudo.


> [!NOTE]
> **💡 Dica**: Para evitar que um efeito sonoro se torne repetitivo demais durante um jogo, é útil randomizar pequenas variações no pitch do som a cada vez que ele é reproduzido.


* `currentTime`: *(opcional)* 0 por padrão. Defina esse valor para evitar começar do início do arquivo de som.

Cada entity só pode ter um único `AudioSource` component, que só pode reproduzir um único clip por vez. Essa limitação pode ser facilmente contornada modificando a source de áudio no momento de reproduzir um novo som, ou incluindo várias entities filhas invisíveis, cada uma com seu próprio som.


> [!WARNING]
> **📔 Nota**: Os sons são reproduzidos na instância local de cada player. Outros players próximos não ouvirão os mesmos sons, a menos que a scene local deles também os reproduza explicitamente.


### Pré-carregando um som

Se uma entity usa um som, mas ele não é reproduzido imediatamente no runtime da scene, talvez leve algum tempo para baixar. Ele pode estar disponível no runtime da scene usando o `AssetLoad` component.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
  ],
})
```

Para mais informações, consulte a documentação de [Pré-carregar recursos](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/pre-load-resources/) .

## Parando sons

Para impedir que uma entity reproduza seu som, use a `AudioSource.stopSound()` função. Você só precisa especificar a entity, já que cada entity tem um único `AudioSource` component, e cada `AudioSource` component reproduz um único arquivo por vez.

```ts
AudioSource.stopSound(sourceEntity)
```

`stopSound()` retorna `false` se a entity não tiver um `AudioSource` component.

Outra forma de parar um som é definir a `playing` propriedade como false.

```ts
// Criar entity
const sourceEntity = engine.addEntity()

// Criar component AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/explosion.mp3',
	playing: true,
})

// Definir uma função simples
function stopSound(entity: Entity) {
	// buscar a versão mutável do component da source de áudio
	const audioSource = AudioSource.getMutable(entity)

	// modificar o valor playing
	audioSource.playing = false
}

// chamar a função
stopSound(sourceEntity)
```

## Detectar quando um som termina

Quando um som sem loop termina de tocar sozinho, o engine define a `playing` propriedade do `AudioSource` component de volta para *false*. O código da sua scene pode ler esse valor para saber quando o som terminou, por exemplo para encadear outro som ou ação logo em seguida.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const audio = AudioSource.get(sourceEntity)
	const isPlaying = audio.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('sound finished playing')
		// reagir aqui: reproduzir outro som, avançar uma sequência, etc.
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 Nota**: Ao verificar o component a cada frame, sempre leia-o com `AudioSource.get()` (somente leitura). Usar `AudioSource.getMutable()` marcaria o component como alterado a cada frame, causando trabalho de sincronização desnecessário.
> 
> O `playing` propriedade só é alterada pelo engine quando o som termina sozinho. Se a scene parar o som explicitamente (via `AudioSource.stopSound()` ou definindo `playing` como *false*), essa alteração vem do seu próprio código, e sons em loop tocam até serem parados, então nunca alteram a propriedade por conta própria.


Alternativamente, use `audioEventsSystem.registerAudioEventsEntity` para definir uma função que é executada toda vez que o estado de reprodução do áudio da entity muda, assim como nos [eventos de vídeo](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/#video-events) para videos. A função recebe um evento com um campo `state` , usando o enum `MediaState` .

```ts
import { engine, AudioSource, audioEventsSystem, MediaState } from '@dcl/sdk/ecs'

// ... Criar sourceEntity com um component AudioSource ...

audioEventsSystem.registerAudioEventsEntity(sourceEntity, (audioEvent) => {
	switch (audioEvent.state) {
		case MediaState.MS_LOADING:
			console.log('audio event - sound is LOADING')
			break
		case MediaState.MS_READY:
			console.log('audio event - sound is READY (loaded, or finished playing)')
			break
		case MediaState.MS_PLAYING:
			console.log('audio event - sound started PLAYING')
			break
		case MediaState.MS_ERROR:
			console.log('audio event - sound ERROR (e.g. the file failed to load)')
			break
	}
})
```

Uma transição de `MS_PLAYING` como `para MS_READY` significa que o som parou de tocar. O enum completo também inclui `MediaState` MS\_NONE `MS_LOADING`, `MS_PAUSED`, `MS_BUFFERING`, `MS_SEEKING`, `MS_ERROR` e `, e cada evento carrega um`timestamp `de quando a mudança de estado ocorreu. Você também pode consultar a última state reportada a qualquer momento com` audioEventsSystem.getAudioState(entity) `, desregistre o callback com`audioEventsSystem.removeAudioEventsEntity(entity) `, ou verifique se uma entity está registrada com`audioEventsSystem.hasAudioEventsEntity(entity) `. O mesmo system também funciona para entities com um`AudioStream [`: Esses recursos são suportados apenas no client Desktop.`](/docs/creator/content-creator-pt/scenes-sdk7/media/audio-streaming/) component.


> [!WARNING]
> **📔 Nota**Reprodução em loop


## Reprodução em loop

Para manter um som tocando em um loop contínuo, defina o `loop` field do `AudioSource` component para *true* antes de começar a reproduzi-lo.

```ts
// Criar entity
const sourceEntity = engine.addEntity()

// Criar component AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	loop: true,
})
```

Sons em loop são especialmente úteis para adicionar música de fundo ou outros sons de fundo.

## Definir volume

Você pode definir o `volume` propriedade do `AudioSource` component para alterar o volume de um som.

O volume é expresso como um número de *0* como *1*.

```ts
// Criar entity
const sourceEntity = engine.addEntity()

// Criar component AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	volume: 0.5,
})
```


> [!WARNING]
> **📔 Nota**: É claro que o volume de um som também é afetado pela distância do player à source de áudio. Conforme o player se afasta, o volume diminui.


## Sons globais

Por padrão, todos os sons de um `AudioSource` são posicionais. Isso significa que eles parecem ser gerados a partir da posição do `Transform` component, e soam mais altos conforme o player se aproxima. Mas você também pode configurar um som para ser global, de modo que o volume seja constante, não importa onde o player esteja. Isso é ideal para usar em música de fundo, sons de notificação e outros sons não posicionais.


> [!WARNING]
> **📔 Nota**: Sons globais são um recurso suportado apenas no client desktop do DCL 2.0.


Para tornar um som global, defina o `global` propriedade para *true*.

```ts
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/music.mp3',
	playing: true,
	global: true,
})
```

Um som global ainda precisa de um `Transform` na sua entity, embora sua posição não afete mais o que o player ouve. Qualquer posição serve.

## Reproduzir um segmento de um som

Para reproduzir um segmento de um arquivo de som mais longo, use a `playSoundSegment()` na biblioteca SDK Utils. Veja [SDK7 Utils](https://github.com/decentraland/sdk7-utils).

Você também pode conseguir isso definindo explicitamente o `currentTime` propriedade em um `AudioSource` component, e depois parando-o após aguardar um período de tempo.

## Streaming de áudio

Veja [Streaming de áudio](/docs/creator/content-creator-pt/scenes-sdk7/media/audio-streaming/) para aprender como você pode reproduzir um stream de áudio ao vivo de uma source externa.

## Análise de áudio

Veja [Análise de áudio](/docs/creator/content-creator-pt/scenes-sdk7/media/audio-analysis/) para aprender como ler dados de amplitude e frequência em tempo real de um `AudioSource` e gerar visuais reativos a partir disso, como cubes que pulsam com o baixo ou visualizadores de barras no estilo equalizador.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
