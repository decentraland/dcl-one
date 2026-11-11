> Faça o avatar do jogador reproduzir animações padrão ou personalizadas, e detecte quando um emote termina

# Animações de Avatar

Você pode fazer o player executar uma animação como parte do código da scene. Isso pode ajudar a proporcionar mais imersão, e também pode ajudar a comunicar o que outros players estão fazendo uns aos outros. As animações do Avatar são vistas tanto pelo player (na visão em 3ª pessoa) quanto por quaisquer outros players ao redor.

As animações controladas pelo player que são executadas no corpo inteiro são substituídas pelas animações padrão de locomoção, como caminhar e pular. Portanto, as animações de corpo inteiro reproduzidas pela scene só são executadas enquanto o player está parado. Se o player andar ou pular, qualquer animação de corpo inteiro é interrompida. As animações da parte superior do corpo não são interrompidas pelo movimento.


> [!WARNING]
> **📔 Nota**: Os players só podem ser animados se já estiverem parados dentro dos limites da scene, e não se estiverem em uma scene vizinha. Smart wearables podem reproduzir animações em qualquer lugar.
> 
> Enquanto um player está executando uma animação, ele não é afetado por colisões, e seus movimentos não são limitados pela física da scene. Observe também que, se uma animação deslocar o player de sua posição original (por exemplo, se a animação envolver um salto), o componente Transform do player não será afetado por esse deslocamento.


### Use o Scene Editor

A maneira mais fácil de fazer um player executar uma animação é usar o Scene Editor. Use a opção sem código **Reproduzir Emote** para reproduzir uma animação padrão, ou a **Reproduzir Emote Personalizado** para reproduzir uma animação de um arquivo. Veja [Torne qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).

### Animações padrão

Use a `triggerEmote()` função para executar uma das animações padrão que os players podem reproduzir em qualquer lugar em Decentraland. Esta função recebe um objeto com as seguintes propriedades como argumento:

* `predefinedEmote`: Um nome de string para um emote existente.
* `mask`: (opcional) Reproduza a animação em apenas parte do corpo do Avatar, usando um valor do `AvatarMask` enum. Por exemplo, `AvatarMask.AM_UPPER_BODY` anima apenas a parte superior do corpo do Avatar. Veja [Animar apenas a parte superior do corpo](#animate-only-the-upper-body).

```ts
import { triggerEmote } from '~system/RestrictedActions'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Dance' },
	},
	() => {
		triggerEmote({ predefinedEmote: 'robot' })
	}
)
```

Os seguintes emotes mostram feedback sobre as ações do player na sua scene; todos eles são valores válidos para o campo `predefinedEmote` :

* `buttonDown`
* `buttonFront`
* `getHit`
* `knockOut`
* `lever`
* `openChest`
* `openDoor`
* `punch`
* `push`
* `swingWeaponOneHand`
* `swingWeaponTwoHands`
* `throw`
* `sittingChair1`
* `sittingChair2`
* `sittingGround1`
* `sittingGround2`

Os quatro `sitting` emotes ficam em loop: o Avatar mantém a pose até que o player se mova ou a sua scene chame `stopEmote({})`. `stopEmote` retorna com `{ success: boolean }`, que é `false` quando o Explorer recusou a chamada, por exemplo porque o player não está mais parado dentro da sua scene. Todos os outros são reproduzidos uma vez e o Avatar volta ao estado inativo.

Estes emotes estão disponíveis para todos os players na roda de emotes padrão deles e também podem ser usados em qualquer scene.

* `wave`
* `fistpump`
* `robot`
* `raiseHand`
* `clap`
* `money`
* `kiss`
* `tik`
* `hammer`
* `tektonik`
* `dontsee`
* `handsair`
* `shrug`
* `disco`
* `dab`
* `headexplode`
* `crafting`
* `handsintheair`
* `Victory`
* `Waving`
* `fistpump_short`


> [!WARNING]
> **📔 Nota**: Use esses nomes exatamente como listados. Um nome incorreto falha silenciosamente na sua scene: nada é reproduzido e nada aparece no console da scene. Tome cuidado com `handsintheair` sem separadores, e `fistpump_short` com seu sufixo.



> [!NOTE]
> **💡 Dica**: Se um player andar ou pular enquanto estiver reproduzindo a animação, ele a interromperá. Se você não quiser que isso seja possível, pode congelar o Avatar com [Input Modifiers](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#freeze-the-player) pela duração da animação do Avatar.


### Animações personalizadas

Use a `triggerSceneEmote()` para fazer o player executar uma animação personalizada, armazenada como um arquivo .glb como parte do asset da scene.


> [!NOTE]
> **💡 Dica**: Você não precisa criar estas sozinho. O **Avatar Animations** pacote Avatar Animations no Scene Editor tem 45 animações prontas, incluindo poses sentadas, pesca, carregar uma caixa e dança. Arraste uma para a visualização 3D e o editor copia seu arquivo para a sua scene, depois passe o caminho desse arquivo para `triggerSceneEmote()`. Os arquivos ficam em `assets/asset-packs/<animation-folder>/`, por exemplo `assets/asset-packs/whack/Whack_Avatar_emote.glb`. Verifique o caminho exato em **Local Assets**. Veja [Adicionar itens](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/#avatar-animations).



> [!WARNING]
> **📔 Nota**: O nome do arquivo **deve** terminar em `_emote.glb` para funcionar como uma animação de Avatar.


Esta função recebe um objeto com as seguintes propriedades:

* `src`: O caminho completo para o arquivo do emote, contado a partir da pasta raiz do seu projeto. Um arquivo em `assets/animations/sit_emote.glb` precisa `'assets/animations/sit_emote.glb'`.
* `loop`: Se true, a animação ficará em loop continuamente até que o player se mova ou a animação seja interrompida. False por padrão.
* `mask`: (opcional) Reproduza a animação em apenas parte do corpo do Avatar, usando um valor do `AvatarMask` enum. Por exemplo, `AvatarMask.AM_UPPER_BODY` anima apenas a parte superior do corpo do Avatar. Veja [Animar apenas a parte superior do corpo](#animate-only-the-upper-body).


> [!WARNING]
> **📔 Nota**: `src` deve ser o caminho completo para o arquivo. Não há busca pelo nome do arquivo, e o `_emote` sufixo não é adicionado para você. Se o caminho estiver errado, `triggerSceneEmote()` retorna `{ success: false }` e nada é reproduzido. Copie o caminho da pasta do seu projeto e inclua todas as pastas nele.


```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import { AvatarMask } from '@dcl/sdk/ecs'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Make snowball' },
	},
	() => {
		triggerSceneEmote({ src: 'assets/animations/Snowball_Throw_emote.glb', loop: false, mask: AvatarMask.AM_UPPER_BODY })
	}
)
```


> [!NOTE]
> **💡 Dica**: Se um player andar ou pular enquanto estiver reproduzindo a animação, ele a interromperá. Se você não quiser que isso seja possível, pode congelar o Avatar com [Input Modifiers](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#freeze-the-player) pela duração da animação do Avatar.


#### Props, partículas e som

Um arquivo de emote reproduzido com `triggerSceneEmote()` pode incluir props, como uma raquete de tênis ou um jato de chamas, animados junto com o Avatar. Crie-o da mesma forma que um emote Emotes 2.0, seguindo as regras de nomenclatura em [Props e Sons](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/props-and-sounds/):

* O arquivo contém dois clips de animação: um cujo nome inclui `_Avatar` para o Avatar, e outro cujo nome inclui `_Prop` para a armature do prop.
* Ambos os clips têm a mesma duração.
* Os efeitos de partículas funcionam se você os converter em bones animados, veja [Particles in Emotes](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/particles-in-emotes/).

Os outros players ao redor veem os props, assim como veem a animação do Avatar.


> [!WARNING]
> **📔 Nota**: `triggerSceneEmote()` não reproduz som. O áudio só é reproduzido para emotes enviados como emotes independentes no Builder. Para associar um som a um emote de scene, reproduza-o com um [AudioSource](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/) ao mesmo tempo.


### Animar apenas a parte superior do corpo

Ambos `triggerEmote()` e `triggerSceneEmote()` aceitam uma `mask` propriedade opcional que limita a animação apenas a parte do corpo do Avatar.

Normalmente, uma animação para assim que o player anda ou pula, porque as animações padrão de locomoção assumem o controle de todo o corpo. Quando você define `mask` para `AvatarMask.AM_UPPER_BODY`, a animação controla o Avatar apenas da cintura para cima, e as pernas continuam sob o controle das animações padrão de locomoção. Isso significa que o player pode continuar andando ou correndo pela scene enquanto a parte superior do corpo reproduz sua animação. Use isso para ações que não devam interromper o movimento, como carregar uma caixa, segurar uma tocha ou fazer malabarismo.

```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import {
	engine,
	AvatarMask,
	InputAction,
	MeshCollider,
	MeshRenderer,
	pointerEventsSystem,
	Transform,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const cheerButton = engine.addEntity()
Transform.create(cheerButton, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(cheerButton)
MeshCollider.setBox(cheerButton)
pointerEventsSystem.onPointerDown(
	{
		entity: cheerButton,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Cheer' },
	},
	() => {
		triggerSceneEmote({
			src: 'assets/animations/Cheer_emote.glb',
			loop: true,
			mask: AvatarMask.AM_UPPER_BODY,
		})
	}
)
```

Neste exemplo, o player clica em um botão e começa a torcer com os braços no ar, em uma animação em loop. Como apenas a parte superior do corpo é animada, ele pode continuar correndo pela scene enquanto torce, por exemplo para acompanhar a ação em uma corrida ou em uma partida de futebol.

Algumas coisas a ter em mente:

* `AvatarMask.AM_UPPER_BODY` é atualmente o único valor no `AvatarMask` enum. Para reproduzir uma animação no corpo inteiro, simplesmente não defina a `mask` propriedade.
* A `loop` propriedade se comporta da mesma forma que nas animações de corpo inteiro: com `loop: false` a animação com máscara é reproduzida uma vez e então a parte superior do corpo volta para a locomoção normal, com `loop: true` ela se repete até ser interrompida.
* Para interromper uma animação mascarada em loop pelo código, chame `stopEmote({})`, também importado de `~system/RestrictedActions`. Retorna com `{ success: boolean }`.

### Detectar quando um emote termina

Cada evento do ciclo de vida de um emote é reportado por meio do `AvatarEmoteCommand` componente na entidade do player. Cada nova entrada carrega um campo `state` , com um valor do `EmoteState` enum:

* `EmoteState.ES_STARTED`: O emote começou a ser reproduzido. Este também é o valor informado quando o `state` campo está ausente.
* `EmoteState.ES_FINISHED`: Um emote sem loop foi reproduzido até seu fim natural.
* `EmoteState.ES_INTERRUPTED`: O emote foi interrompido antes do fim: o player se moveu ou pulou, teleportou-se, outro emote começou, o emote foi parado explicitamente ou o player saiu da scene.

Isso funciona para emotes acionados pela scene (`triggerEmote()` e `triggerSceneEmote()`) assim como para emotes que o player reproduz por conta própria pela roda de emotes, e também para emotes reproduzidos por outros players na scene.

Use a `onChange` função no `AvatarEmoteCommand` componente para reagir a cada nova entrada:

```ts
import { AvatarEmoteCommand, EmoteState } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return

		switch (emote.state ?? EmoteState.ES_STARTED) {
			case EmoteState.ES_STARTED:
				console.log('emote started: ', emote.emoteUrn)
				break
			case EmoteState.ES_FINISHED:
				console.log('emote finished naturally: ', emote.emoteUrn)
				break
			case EmoteState.ES_INTERRUPTED:
				console.log('emote was interrupted: ', emote.emoteUrn)
				break
		}
	})
}
```


> [!WARNING]
> **📔 Nota**: Este recurso é suportado apenas no cliente Desktop. Emotes reproduzidos com um `mask` (emotes de corpo parcial) no player local atualmente não reportam eventos do ciclo de vida.



> [!NOTE]
> **💡 Dica**: Para reproduzir animações em Avatars que não são players, veja [NPC Avatars](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars/#animations).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
