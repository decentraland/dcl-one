> Transmita vídeo para uma scene

# Reprodução de Vídeo

Há três maneiras diferentes de mostrar um vídeo em uma scene:

* Fazer upload de um arquivo de vídeo como parte do conteúdo da scene
* Fazer stream do vídeo a partir de uma fonte externa
* Fazer stream ao vivo via Decentraland cast


> [!NOTE]
> **💡 Dica**: No [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), você pode usar um **Video Player** [Smart Item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para uma forma sem código de conseguir isso.


Em todos os casos, você precisará de:

* Uma entity com uma [forma primitiva](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/) como um plano, cubo ou até mesmo um cone.
* A [material](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/) com um `VideoTexture` atribuído à sua textura
* A `VideoPlayer` component para controlar o estado do vídeo.

## Considerações de desempenho

Tenha em mente que fazer stream de vídeo exige um esforço significativo da máquina do player. É recomendável evitar reproduzir mais de um vídeo por vez.

Se muitos vídeos estiverem sendo reproduzidos ao mesmo tempo em sua scene, alguns serão pausados pelo engine. A prioridade para pausar uma tela é determinada com base em vários fatores, incluindo proximidade do player, tamanho e se a tela está no campo de visão do player. O número máximo de vídeos simultâneos depende das configurações de qualidade do player.

* Baixa: 1
* Média: 5
* Alta: 10

Também recomendamos começar a reproduzir o vídeo quando o jogador estiver perto ou executar uma ação para isso. Começar a reproduzir um vídeo quando sua cena é carregada muito ao longe no horizonte afetará desnecessariamente o desempenho enquanto os jogadores visitam as cenas vizinhas.

Evite também fazer stream de vídeos em resolução muito alta. Não use nada acima de *HD*.

Também é ideal reproduzir vídeos em materiais Basic (unlit), para reduzir a carga de desempenho, como é o caso em todos os trechos de exemplo abaixo.

## Mostrar um vídeo

As instruções a seguir se aplicam às três opções de exibição de vídeo:

1. Crie uma entity para servir como tela de vídeo. Dê a essa entity um `MeshRenderer` component para que ela tenha uma forma visível.
2. Crie um `VideoPlayer` component, referenciando uma URL de stream ou um caminho para um arquivo de vídeo. Aqui você também pode definir o `playing` estado do vídeo e seu volume. Esse component pode ser atribuído à entity da tela de vídeo ou a qualquer outra entity na scene.
3. Crie um `VideoTexture` object, e em sua `videoPlayerEntity` property atribua a entity que possui o `VideoPlayer` componente.
4. Crie um `Material`, atribua-o à entity da tela e defina sua `textura` até `VideoTexture` que você criou.

Este exemplo usa um vídeo que está armazenado localmente em uma `/videos` pasta no projeto da scene:

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: 'videos/myVideo.mp4',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

Para usar um vídeo de uma URL de stream externa, altere a etapa 2 para que o `src` property no `VideoPlayer` component referencie a URL de stream.

```ts
// #2
VideoPlayer.create(screen, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})
```

Veja [Streaming ao vivo](#live-streaming) para detalhes sobre como usar este terceiro método alternativo.

## Sobre Streaming Externo

A origem do streaming deve ser uma *https* URL (*http* (URLs não são suportadas).

Você deve conseguir colar uma URL que aponte para um vídeo da maioria dos sites de streaming de vídeo mais populares. Tenha cuidado com os termos de serviço dessas plataformas.

Para fazer stream de um arquivo de vídeo que você tem na sua máquina local, o caminho mais fácil é fazer upload desse vídeo para um Google Drive público e colar o link.

Outra opção é usar um provedor de hospedagem gerenciada como [Vimeo](https://vimeo.com/) , [Livepeer Studio](https://livepeer.studio/) ou [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) onde você paga uma taxa ao provedor para gerenciar toda a infraestrutura de streaming.

A abordagem mais poderosa é configurar seu próprio server, usando software gratuito, mas pagando pela hospedagem em uma plataforma como [Digital Ocean](https://try.digitalocean.com/developerbrand/?_campaign=emea_brand_kw_en_cpc&_adgroup=digitalocean_exact_exact&_keyword=digitalocean&_device=c&_adposition=&_content=conversion&_medium=cpc&_source=bing&msclkid=160bfc160a2a1bab9bbf9933594bd9c5&utm_source=bing&utm_medium=cpc&utm_campaign=emea_brand_kw_en_cpc&utm_term=digitalocean&utm_content=DigitalOcean%20Exact_Exact) ou [Cloudflare](https://www.cloudflare.com/products/cloudflare-stream/). Você pode implantar algo como um [Node Media Server](https://github.com/illuspas/Node-Media-Server), que fornece a maior parte do que você precisa já pronto.

Todas essas opções têm prós e contras para diferentes cenários. Você deve avaliar o que é melhor para você levando em conta suas necessidades, habilidades técnicas e orçamento.

## Configurando o OBS para um streaming bem-sucedido

[OBS](https://obsproject.com/) é uma ferramenta popular e gratuita para gerenciar seus streams.

Quer você esteja usando a stream key de um venue ou seu próprio server RTMP, suas configurações no OBS são importantes para o sucesso do seu stream. Você deve buscar uma conexão sólida e consistente.

### Configuração simples do OBS

A seguinte configuração simples é recomendada:

* Bitrate 2500kbps (o que funcionará com todos os venues do Decentraland)
* Bitrate de áudio 160kbps
* Predefinição do encoder de vídeo: Hardware NVENC
* Encoder de áudio AAC
* Resolução mais ampla: 720 (qualquer valor maior causa problemas no DCL)
* Taxa de quadros 30fps


> [!WARNING]
> **📔 Nota**: Certifique-se de desativar o Multitrack streaming no OBS. Você pode encontrar isso em Settings > Stream.


### Conselhos para novos streamers

* Verificações iniciais de áudio são essenciais para testar sua configuração com o venue.
* Pequenos erros, como um dígito errado na stream key, são os mais prováveis de atrapalhar o stream.
* Não ultrapasse resolução 720 nem um bitrate de 2500 kbps.

## Streaming ao vivo

Você pode fazer livestream da sua câmera ou compartilhar sua tela usando o [Streaming ao vivo](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming/) recurso do [ferramentas de admin](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/scene-admin/) smart item.

Este método de streaming usa a mesma arquitetura de comunicação usada para comunicações ao vivo entre players. É fácil de configurar e tem muito menos atraso do que fazer stream a partir de fontes externas.

1. Adicione um [ferramentas de admin](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/scene-admin/) smart item à sua scene, bem como um [Video player](/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/) smart item.
2. Publique sua scene, seja em um World ou em Genesis City.
3. Entre na scene como um player com permissão para usar as ferramentas de Admin.
4. Abra o console de Admin, selecione a **Video** aba e então selecione a funcionalidade **Live** e clique em **Get Stream Key** botão.
5. Copie o **Server URL** e *Streaming key*\* para o seu software de streaming (por exemplo, OBS).
6. Pressione o **Ativar** botão para iniciar o streaming.

Em vez de adicionar um smart item de Video player à sua scene, você também pode usar a URL `livekit-video://current-stream` como source de vídeo, para reproduzir o stream em sua scene. Ainda assim, você precisará das ferramentas de Admin para obter a stream key.

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: `livekit-video://current-stream`,
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

## Materiais de vídeo

Na maioria das vezes, você vai querer reproduzir vídeos em um [material Basic](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#unlit-materials)unlit, em vez de um material PBR. Isso resulta em uma imagem muito mais brilhante e nítida e é melhor para o desempenho.

```ts
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

Geralmente é recomendado reproduzir vídeos em materiais Basic unlit, pois isso é melhor para o desempenho. No entanto, se você quiser projetar um vídeo em um material PBR, tenha em mente que as propriedades padrão fazem o vídeo parecer bastante opaco. Você pode melhorar isso alterando outras propriedades do material. Aqui estão algumas configurações recomendadas para destacar mais o vídeo:

```ts
Material.setPbrMaterial(screen, {
	texture: videoTexture,
	roughness: 1.0,
	specularIntensity: 0,
	metallic: 0,
	emissiveTexture: videoTexture,
	emissiveIntensity: 0.6,
	emissiveColor: Color3.White(),
})
```


> [!NOTE]
> **💡 Dica**: Como o vídeo é uma textura adicionada a um material, você também pode experimentar outras propriedades dos materiais, como tingi-lo com uma cor ou adicionar outras camadas de textura, por exemplo para produzir um efeito de tela suja.
> 
> Veja [materials](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/) para mais detalhes.


## Sobre arquivos de vídeo

Os seguintes formatos de arquivo são suportados:

* *.mp4*
* *.ogg*
* *.webm*

Tenha em mente que um arquivo de vídeo aumenta o tamanho total da cena, o que faz com que a cena demore mais para ser baixada pelos jogadores que entrarem nela. O tamanho do vídeo também pode fazer com que você ultrapasse as [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/), já que você tem um máximo de 15 MB por parcel para usar. Recomendamos comprimir o vídeo o máximo possível, para que isso seja menos problemático.

Também recomendamos começar a reproduzir o vídeo quando o player estiver por perto ou realizar uma ação para acioná-lo. Começar a reproduzir um vídeo quando sua scene estiver carregada longe no horizonte afetará desnecessariamente o desempenho enquanto os players visitam scenes vizinhas.

## Iniciar, pausar e parar um vídeo

Para começar a reproduzir o vídeo ou pausá-lo, defina a `playing` property para *true* ou *false*. Se `playing` estiver definido como false, o vídeo fica pausado no último frame exibido.

Você pode tornar uma tela alternável adicionando um event de pointer a ela, como mostrado abaixo:

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Reproduzir/Pausar' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = !videoPlayer.playing
	}
)
```

Para parar o vídeo e enviá-lo de volta ao primeiro frame, defina a `position` property para 0. No exemplo a seguir, clicar no vídeo o interrompe.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'PARAR' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = false
		videoPlayer.position = 0
	}
)
```

## Configurar o video player

As seguintes properties opcionais estão disponíveis para definir no `VideoPlayer` component:

* `playing`: Determina se o vídeo está sendo reproduzido no momento. Se false, o vídeo fica pausado.


> [!WARNING]
> **📔 Nota**: Apenas um número limitado de vídeos pode ser reproduzido ao mesmo tempo, dependendo das configurações de qualidade do player. Se a scene reproduzir mais do que isso, o engine pausa os de menor prioridade. Veja [Considerações de desempenho](#performance-considerations).


* `playbackRate`: Altera a velocidade com que o vídeo é reproduzido. *1* por padrão.
* `volume`: Permite alterar o volume do áudio. *1* por padrão.
* `position`: Permite definir uma posição inicial diferente no vídeo. Ela é expressa em segundos após o início original do vídeo. *0* por padrão, o que faz com que ele comece no início real do vídeo.
* `loop`: Boolean que determina se o vídeo é reproduzido continuamente em loop ou se ele para após ser reproduzido uma vez. *false* por padrão.

## Reproduzir vários vídeos

Para evitar problemas de desempenho, mantenha baixo o número de vídeos diferentes sendo reproduzidos ao mesmo tempo; o engine permite apenas um número limitado de vídeos simultâneos, dependendo das configurações de qualidade do player (veja [Considerações de desempenho](#performance-considerations)). No entanto, uma scene pode reproduzir várias cópias da mesma textura de vídeo em várias telas diferentes. Isso não é restrito, pois impacta o desempenho consideravelmente menos do que reproduzir vídeos separados. Para reproduzir o mesmo vídeo em várias entities, basta atribuir a mesma instância do objeto de textura de vídeo aos `Material` components de cada entity de tela.

```ts
// #1
const screen1 = engine.addEntity()
MeshRenderer.setPlane(screen1)
Transform.create(screen1, { position: { x: 4, y: 1, z: 4 } })

const screen2 = engine.addEntity()
MeshRenderer.setPlane(screen2)
Transform.create(screen2, { position: { x: 6, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen1, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen1 })

// #4
Material.setBasicMaterial(screen1, {
	texture: videoTexture,
})

Material.setBasicMaterial(screen2, {
	texture: videoTexture,
})
```

Observe que, no exemplo acima, só é necessário criar um `VideoPlayer` component, que controla o estado das duas telas de vídeo. Nesse caso, o component é atribuído à entity `screen1` entity, mas também poderia ser atribuído a qualquer outra entity na scene, não necessariamente a uma das telas.

## Eventos de vídeo

Lide facilmente com mudanças de estado em um vídeo, para responder quando um vídeo começa a ser reproduzido, é pausado, etc. Isso pode ser usado, por exemplo, para reproduzir animações em perfeita sincronia com um vídeo, garantindo que comecem ao mesmo tempo que o vídeo.

Use `videoEventsSystem.registerVideoEventsEntity` para definir uma função que é executada toda vez que o estado do vídeo atribuído a uma entity muda. Sempre que o estado mudar, sua função pode verificar o novo estado e responder de acordo.

```ts
import {
	engine,
	Entity,
	VideoPlayer,
	videoEventsSystem,
	VideoState,
} de '@dcl/sdk/ecs'

// ... Crie videoPlayerEntity com component VideoPlayer, Transform, MeshRenderer.setPlane(), etc. ...

videoEventsSystem.registerVideoEventsEntity(
	videoPlayerEntity,
	function (videoEvent) {
		console.log(
			'evento de vídeo - estado: ' +
				videoEvent.state +
				'\noffset atual:' +
				videoEvent.currentOffset +
				'\nduração do vídeo:' +
				videoEvent.videoLength
		)

		switch (videoEvent.state) {
			case VideoState.VS_READY:
				console.log('evento de vídeo - vídeo está PRONTO')
				break
			case VideoState.VS_NONE:
				console.log('evento de vídeo - vídeo está SEM ESTADO')
				break
			case VideoState.VS_ERROR:
				console.log('evento de vídeo - ERRO no vídeo')
				break
			case VideoState.VS_SEEKING:
				console.log('evento de vídeo - vídeo está PROCURANDO')
				break
			case VideoState.VS_LOADING:
				console.log('evento de vídeo - vídeo está CARREGANDO')
				break
			case VideoState.VS_BUFFERING:
				console.log('evento de vídeo - vídeo está NO BUFFER')
				break
			case VideoState.VS_PLAYING:
				console.log('evento de vídeo - vídeo começou a REPRODUZIR')
				break
			case VideoState.VS_PAUSED:
				console.log('evento de vídeo - vídeo está PAUSADO')
				break
		}
	}
)
```

O objeto videoEvent passado como entrada para a função contém as seguintes properties:

* `currentOffset` (*number*): O valor atual da propriedade `position` do vídeo. Este valor mostra os segundos após o início original do vídeo. *-1* se o vídeo ainda não tiver começado a ser reproduzido.
* `state`: O novo estado do vídeo, expresso como um valor de `VideoState` . Este enum pode conter os seguintes valores possíveis:
  * `VideoState.VS_READY`
  * `VideoState.VS_NONE`
  * `VideoState.VS_ERROR`
  * `VideoState.VS_SEEKING`
  * `VideoState.VS_LOADING`
  * `VideoState.VS_BUFFERING`
  * `VideoState.VS_PLAYING`
  * `VideoState.VS_PAUSED`
* `videoLength` (*number* ): A duração em segundos de todo o vídeo. *-1* se a duração for desconhecida.
* `timestamp` ( *number*): Um *lamport* timestamp que é incrementado toda vez que o vídeo muda de estado.
* `tickNumber` (*number*): O momento em que o event ocorreu, expresso como a contagem de ticks desde que a scene começou a ser executada.

Se você registrar novamente um callback em uma entity que já possui um (por exemplo, para alterar o que acontece na mudança de estado), o novo callback substitui o antigo e o último estado reportado é preservado. Isso significa que o novo callback não é chamado novamente para um estado que já foi reportado ao callback anterior.

### Verificar se uma entity está registrada

Use `videoEventsSystem.hasVideoEventsEntity()` para verificar se uma entity já possui um callback de eventos de vídeo registrado.

```ts
if (videoEventsSystem.hasVideoEventsEntity(videoPlayerEntity)) {
	console.log('Esta entity já possui um listener de eventos de vídeo')
}
```

### Último evento de vídeo

Consulte um vídeo para sua última mudança de estado usando `videoEventsSystem.getVideoState()`. Isso retorna o mais recente `VideoEvent` value para a entity, ou `undefined` se nenhum event tiver sido reportado ainda. Você pode chamar isso a partir de um system para consultar o estado do vídeo a cada frame sem registrar um callback.

```ts
function mySystem() {
	const latestVideoEvent = videoEventsSystem.getVideoState(videoPlayerEntity)
	if (!latestVideoEvent) return

	console.log(`estado: ${latestVideoEvent.state}
    \noffsetAtual: ${latestVideoEvent.currentOffset}
    \nduraçãoDoVídeo: ${latestVideoEvent.videoLength}`)
}
```

## Máscaras alpha em vídeos

Um truque interessante para ter telas de vídeo não retangulares é aplicar uma alpha texture sobre um plano. Você pode recortar parte do plano para qualquer formato que quiser.

Use a imagem a seguir para recortar seu vídeo em um formato circular, com cantos transparentes.

![](/docs/_remote/1f517d241f0753831e85b88c.webp)

```ts
const videoTexture = Material.Texture.Video({
	videoPlayerEntity: screen,
})
const alphaMask = Material.Texture.Common({
	src: 'assets/scene/circle_mask.png',
	wrapMode: TextureWrapMode.TWM_MIRROR,
})

Material.setBasicMaterial(screen, {
	texture: videoTexture,
	alphaTexture: alphaMask,
})
```

![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)


> [!WARNING]
> **📔 Nota**: Em versões anteriores, a `alphaTexture` property estava presente apenas em materiais PBR. Atualmente, ela só funciona em materiais básicos.


## Reproduzir um vídeo em um modelo glTF

Você pode reproduzir um vídeo em um *glTF* modelo usando [GltfNodeModifiers](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) componente. Veja [Modificar materiais glTF](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) para mais detalhes.

Isso permite reproduzir seus vídeos em qualquer forma, não apenas em planos. Por exemplo, você pode reproduzir vídeos em uma tela curva ou até mesmo no corpo inteiro de um NPC.

```ts
const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

VideoPlayer.create(myEntity, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						texture: Material.Texture.Video({
							videoPlayerEntity: myEntity,
						}),
					},
				},
			},
		},
	],
})
```

O mapeamento do vídeo seguirá o mapeamento UV original usado pelo modelo. Isso significa que, se o modelo tiver uma textura mapeada para uma parte específica do modelo, o vídeo será mapeado para essa mesma parte.

Você também pode usar o `GltfNodeModifiers` component para reproduzir um vídeo apenas em uma mesh específica dentro do modelo. Por exemplo, você pode reproduzi-lo em uma parede específica de um edifício, mesmo que o modelo abranja o edifício inteiro. Veja [Modificar materiais glTF](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) para mais detalhes.

## Áudio espacial

Por padrão, o vídeo de um `VideoPlayer` component é global, o que significa que ele será ouvido em um volume consistente em toda a sua scene. Se um player sair da scene, ele não ouvirá o streaming de forma alguma.

Para tornar o áudio espacial, defina o `spatial` property para *true*.

```ts
VideoPlayer.create(entity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
})
```

O vídeo agora será ouvido a partir da posição da entity que possui o `VideoPlayer` component, e ficará mais alto conforme o player se aproximar dele.

Controle o áudio espacial com as seguintes properties:

* `spatialMinDistance`: A distância mínima na qual o áudio se torna espacial. Se o player estiver mais perto, o áudio será ouvido em volume máximo. *0* por padrão.
* `spatialMaxDistance`: A distância máxima na qual o áudio é ouvido. Se o player estiver mais longe, o áudio será ouvido com volume 0. *60* por padrão

```ts
const videoPlayerEntity = engine.addEntity()

Transform.create(videoPlayerEntity, {
	position: Vector3.create(8, 2, 8),
})

VideoPlayer.create(videoPlayerEntity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
	spatialMinDistance: 5,
	spatialMaxDistance: 10,
})

MeshRenderer.setPlane(videoPlayerEntity)

Material.setBasicMaterial(videoPlayerEntity, {
	texture: Material.Texture.Video({ videoPlayerEntity: videoPlayerEntity }),
})
```


> [!WARNING]
> **📔 Nota**: Alguns formatos de vídeo não suportam áudio espacial. Certifique-se de que o stream seja codificado em *mp4*, *m4a*, ou *mov*.


## Análise de áudio

Você pode ler dados de amplitude e frequência em tempo real da faixa de áudio de uma `VideoPlayer` entity para criar visuais reativos que sincronizam com a trilha sonora do vídeo. Veja [Análise de áudio](/docs/creator/content-creator-pt/scenes-sdk7/media/audio-analysis/).


> [!NOTE]
> **💡 Dica**: Para ver vídeo reproduzido em um modelo GLTF em vez de uma primitiva, veja o [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier) cena de teste, que direciona um stream HLS para nós GLTF específicos usando `GltfNodeModifiers` texturas de vídeo.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/media/video-playing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
