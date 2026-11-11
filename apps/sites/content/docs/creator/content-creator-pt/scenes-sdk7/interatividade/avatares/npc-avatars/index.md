> Exiba e controle o avatar do NPC

# Avatares NPC

Exiba um avatar como uma entidade numa cena.


> [!NOTE]
> **💡 Dica**: Experimente a [biblioteca NPC Toolkit](https://github.com/decentraland-scenes/dcl-npc-toolkit) para uma experiência mais fácil ao lidar com NPCs, especialmente se precisar interagir através de uma árvore de conversação.


## Criar um avatar

O seguinte trecho cria um avatar com wearables e body shape aleatórios, e nome "NPC".

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar)

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

Ao passar dados para gerar um `AvatarShape`, os seguintes campos são obrigatórios:

* `id`: (obrigatório) Identificador interno do Avatar. Os últimos 4 caracteres do `id` são mostrados após o nome do NPC, em texto esbatido, como o sufixo da Wallet de um jogador. Um `id` com menos de 4 caracteres mostra `0000`. Dê a cada NPC um `id` cujo final fique bem ao lado do seu nome, por exemplo `'guide-0001'`.

Os seguintes campos opcionais também estão disponíveis:

* `nome`: Nome a exibir acima da cabeça do Avatar. Padrão: "NPC". Defina-o como uma string vazia para não mostrar qualquer nome, o que é útil quando quer apenas uma [placa de nome do Avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-nametags/) acima do NPC.
* `bodyShape`: String para definir qual body shape usar. As opções válidas são 'urn:decentraland:off-chain:base-avatars:BaseMale' e 'urn:decentraland:off-chain:base-avatars:BaseFemale'.
* `wearables`: Array com uma lista de URNs dos wearables que o avatar tem atualmente. Se os wearables entrarem em conflito (por exemplo, se dois deles forem chapéus), o último da lista substitui o outro.
* `emotes`: Array com uma lista de URNs para emotes de NFT que o avatar é capaz de reproduzir
* `eyeColor`: *Color3* para a cor dos olhos (qualquer cor é válida)
* `skinColor`: *Color3* para a cor da pele (qualquer cor é válida)
* `hairColor`: *Color3* para a cor do cabelo (qualquer cor é válida)
* `talking`: Se *true*, exibe um conjunto de barras verdes junto ao nome, como quando os jogadores usam chat de voz no mundo.
* <div data-gb-custom-block data-tag="hint" data-style="info" class="hint hint-info"><p><strong>💡 Dica</strong>: Vê <a href="/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/">tipos de cor</a> para mais detalhes sobre como definir cores.</p></div>


> [!WARNING]
> **📔 Nota**: O `AvatarShape`component deve ser importado via
> 
> > `import { AvatarShape } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar facilmente com isso.



> [!WARNING]
> **📔 Nota**: Os campos URN devem seguir o mesmo formato usado para [NFTShapes](/docs/creator/content-creator-pt/scenes-sdk7/media/display-a-certified-nft/): `urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`


## Mover um NPC

Para mover um NPC, altere o `position` do seu `Transform`. `rotation` que definir em `Transform` só é aplicado quando chega.

```ts
// O NPC caminha da sua posição atual para a nova
Transform.getMutable(myAvatar).position = Vector3.create(10, 0.25, 5)
```

Para controlar o movimento manualmente, use um [Tween](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/). Um Tween na posição substitui esta caminhada automática, e um Tween na rotação impede que o NPC se vire para encarar para onde anda. Uma entidade só pode ter um Tween de cada vez, por isso não pode fazer tween da posição e da rotação da mesma entidade ao mesmo tempo.

## Animações

Os avatares reproduzem animações idle padrão quando estão parados.

Para reproduzir animações no avatar, defina o `expressionTriggerId` string com o nome da animação que quer reproduzir.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'dancer-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'robot',
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

O `expressionTriggerId` campo suporta todas as [animações padrão](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#default-animations), bem como animações personalizadas [de um ficheiro de cena](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#custom-animations), e até URNs de emotes publicados no Marketplace.

Para usar uma animação personalizada, indique o caminho para o `_emote.glb` ficheiro na pasta raiz do seu projeto, por exemplo `'assets/animations/Cheer_emote.glb'`. Tal como com [`triggerSceneEmote()`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#custom-animations), um caminho incorreto não reproduz nada.


> [!WARNING]
> **📔 Nota**: `AvatarShape` não tem `mask` campo, por isso uma `expressionTriggerId` animação reproduz-se sempre em todo o corpo. [Animar apenas a parte superior do corpo](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#animate-only-the-upper-body) está disponível no avatar do próprio jogador, não em NPCs.


### Mover um NPC enquanto reproduz um emote

Escrever a `Transform.position`, ou executar um [Tween](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/) sobre ele, cancela o emote que está a ser reproduzido. O avatar volta ao seu ciclo de caminhada, porque o Decentraland calcula a partir de quão longe o avatar se moveu desde o último frame qual a animação de locomoção a misturar.

Para mover um NPC durante um emote, se o movimento abranger uma área pequena talvez queira incorporar o movimento na própria animação, como movimento de root ou hips dentro do `_emote.glb` ficheiro.

### Animações em loop

As animações num `AvatarShape` reproduzem-se uma vez; se quiser que o avatar continue a repetir uma animação em loop, deve criar um system que lhe diga para reproduzir a animação novamente de poucos em poucos segundos.

A única exceção são os emotes padrão em loop, `sittingChair1`, `sittingChair2`, `sittingGround1` e `sittingGround2`, que mantêm a pose num `AvatarShape` sem necessidade de as reproduzir novamente. Uma animação personalizada de um ficheiro nunca entra em loop num `AvatarShape`, mesmo que o clip em si esteja marcado como em loop, por isso uma pose personalizada sustentada precisa do padrão de repetição abaixo.

Use o `expressionTriggerTimestamp` para reproduzir novamente o mesmo emote. O valor deste campo é um [timestamp de Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp), o que significa que não é um valor temporal, mas sim um índice que aumenta em 1 a cada repetição do emote.

Assim, da primeira vez que reproduz um emote, define `expressionTriggerTimestamp` como *0*. Para reproduzir o emote novamente, tem de atualizar este valor para 1. É assim que o engine sabe que esta é uma nova instrução e não uma instrução sobre a qual já agiu.

O seguinte trecho cria um system que executa o mesmo emote a cada 2 segundos:

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'fan-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'clap',
    expressionTriggerTimestamp: 0
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})

let clapTimer = 0
let emoteDuration = 2  // 2 segundos

// system
engine.addSystem((dt: number) => {
    clapTimer += dt
      
    if (clapTimer >= emoteDuration) {
        // Aciona o emote de aplauso
        AvatarShape.getMutable(myAvatar).expressionTriggerTimestamp += 1
        
        clapTimer = 0 // Reiniciar temporizador
    }
})
```


> [!NOTE]
> **💡 Dica**: Tem de saber a duração do emote e fazer dessa a duração do system. Se criar um emote que mantém o avatar imóvel numa mesma pose, é recomendável tornar a duração do emote mais longa do que a do system. Assim, pode garantir que não existem artefactos ao terminar e reiniciar a animação.


## Adicione uma etiqueta acima do nome

Use o `AvatarNametag` component para mostrar uma placa com texto personalizado acima do nome do NPC, por exemplo um cargo como "Shopkeeper" ou "Boss". Anexe-a à mesma entidade que tem o `AvatarShape`:

```ts
AvatarNametag.create(myAvatar, { label: 'Shopkeeper' })
```

Se o `AvatarShape` tiver um `nome`, apenas a placa é mostrada, sem nenhuma caixa vazia de nametag por baixo. Isto é útil para rotular um NPC apenas com um título.


> [!NOTE]
> **💡 Dica**: Para uma placa codificada por cores sem texto, defina `label` para uma string de espaços e escolha uma `backgroundColor`. Os espaços são preservados, por isso mais espaços tornam a placa mais larga.
> 
> ```ts
> AvatarNametag.create(myAvatar, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


Veja [Placas de nome do Avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-nametags/) para a lista completa de campos, incluindo cores.

## Copiar wearables do jogador

O seguinte trecho altera os wearables e outras características de um avatar NPC para corresponder aos que o jogador tem atualmente. Isto pode ser usado numa cena como manequim, para exibir um wearable ou emote específico combinado com o vestuário atual do jogador.

```ts
import { getPlayer } from '@dcl/sdk/src/players'


export function swapAvatar(avatar: Entity) {

  let userData = getPlayer()
  console.log(userData)

  if (!userData || !userData.wearables) return

  const mutableAvatar = AvatarShape.getMutable(avatar)

  mutableAvatar.wearables = userData.wearables
  mutableAvatar.bodyShape = userData.avatar?.bodyShapeUrn
  mutableAvatar.eyeColor = userData.avatar?.eyesColor
  mutableAvatar.skinColor = userData.avatar?.skinColor
  mutableAvatar.hairColor = userData.avatar?.hairColor
  
}
```

## Exibir apenas wearables

Use o `showOnlyWearables` campo para exibir apenas os wearables listados de um avatar. O resto do corpo do avatar ficará invisível.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'mannequin-0001',
	emotes: [],
	wearables: [
    'urn:decentraland:matic:collections-v2:0x90e5cb2d673699be8f28d339c818a0b60144c494:0'
  ],
	showOnlyWearables: true,
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

Isto é útil para exibir wearables, por exemplo numa loja.


> [!NOTE]
> **💡 Dica**: Se um wearable for bastante pequeno, tente definir o `scale` do `Transform` para um valor maior.


## Anexar uma entidade a um NPC

Pode usar o `AvatarAttach` feature para fixar uma entidade a um dos bones de um avatar NPC, por exemplo para que o NPC esteja a segurar um objeto na mão. A entidade mover-se-á juntamente com o avatar quando este animar.

Para usar esta feature, use a `id` propriedade no `AvatarShape` para atribuir um id arbitrário a este avatar e depois referenciar esse id no `AvatarAttach`. O id pode ser qualquer string que quiser.

```ts
// Criar NPC, com um ID
const myAvatar = engine.addEntity()
Transform.create(myAvatar, {
  position: Vector3.create(8, 0.25, 8),
})
AvatarShape.create(myAvatar, {
  id: "my-avatar-id", 
  wearables: [],
  emotes: []
})

// Criar objeto para anexar ao NPC
const attachedEntity = engine.addEntity()
Transform.create(attachedEntity, {
    position: Vector3.create(4, 2, 4),
    scale: Vector3.create(0.15,0.15,0.15)
})
MeshRenderer.setBox(attachedEntity)
Material.setBasicMaterial(attachedEntity, { diffuseColor: Color4.Blue() })
AvatarAttach.create(attachedEntity, {
    avatarId: "my-avatar-id",
    anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND
})
```

Saiba mais sobre o `AvatarAttach` componente [aqui](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#attach-an-entity-to-an-avatar).

## Substituir o avatar do jogador

Não pode alterar quais wearables o avatar do jogador está a usar, mas pode, em alternativa, trocar o avatar do jogador por um avatar NPC que pode personalizar totalmente.

Para fazer isso, esconda o avatar do próprio jogador com uma [Avatar Modifier Area](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/modifier-areas/#hide-avatars) que usa o `AMT_HIDE_AVATARS` modifier, e exiba uma `AvatarShape` entidade na posição do jogador em vez disso.


> [!WARNING]
> **📔 Nota**: Para permitir que o jogador tenha controlo total sobre esse avatar, deve ouvir eventos de button para detetar quando carrega num botão e, em seguida, acionar a animação correspondente no avatar NPC. Veja [Eventos de botão](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/) para mais detalhes.
> 
> A fluidez do controlo pode não ser perfeita ao fazer isto; talvez queira usar isto apenas em casos muito específicos.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
