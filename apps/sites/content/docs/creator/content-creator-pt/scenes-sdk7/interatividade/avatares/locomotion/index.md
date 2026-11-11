> Altere a velocidade e a altura do salto do jogador, ou restrinja como ele pode se mover

# Configurações e Restrições de Locomoção

Você pode alterar como o avatar do player se move enquanto estiver dentro da sua scene. Use o `AvatarLocomotionSettings` component para ajustar velocidades e alturas de salto, e o `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` component para congelar o player ou bloquear tipos específicos de movimento. Ambos os components são colocados em `engine.PlayerEntity`, e ambos só têm efeito enquanto o player estiver dentro dos limites da sua scene.

## Configurações de locomoção

Você pode afetar a locomoção do player, como a velocidade de corrida, a altura do salto e muito mais. Isso pode ser alterado dinamicamente, por exemplo, para permitir que um player colete um aumento temporário de velocidade ao interagir com um item, ou para desativar a capacidade do player de saltar por um curto período de tempo.


> [!NOTE]
> **💡 Dica**: Você pode definir esses valores sem escrever código. Arraste o **Configurações de locomoção** smart item para dentro da sua scene na [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/) e mova seus sliders. Veja [Configurações de locomoção](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/#locomotion-settings).


Para fazer isso, adicione um `AvatarLocomotionSettings` componente ao `engine.PlayerEntity`.

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	jumpHeight: 2,
})
```

As seguintes propriedades estão disponíveis:

* `walkSpeed`: A velocidade na qual o player anda, em metros por segundo. No cliente desktop, os players andam pressionando a tecla control.
* `jogSpeed`: A velocidade na qual o player trota, em metros por segundo. Esta é a forma padrão como o player se move.
* `runSpeed`: A velocidade na qual o player corre, em metros por segundo. No cliente desktop, os players correm pressionando a tecla shift.
* `jumpHeight`: A altura em que o player salta, em metros.
* `runJumpHeight`: A altura em que o player salta depois de correr, em metros.
* `doubleJumpHeight`: A altura do segundo salto ao fazer double-jump, em metros.
* `glidingSpeed`: A velocidade horizontal com que o player se move enquanto plana, em metros por segundo.
* `glidingFallingSpeed`: A velocidade máxima de queda do player enquanto plana, em metros por segundo. Isso limita apenas a descida do player: movimento ascendente, como um impulso de uma força contínua da scene, não é limitado.
* `hardLandingCooldown`: O tempo de recarga após um pouso forte, em segundos. Este é o tempo que o player precisa esperar antes de poder se mover novamente depois de cair de uma altura grande.

Como referência, aqui estão os valores padrão dessas propriedades:

* `walkSpeed`: 1.5 m/s
* `jogSpeed`: 8 m/s
* `runSpeed`: 10 m/s
* `glidingSpeed`: 6 m/s
* `glidingFallingSpeed`: 1 m/s
* `jumpHeight`: 1 m
* `runJumpHeight`: 1.5 m
* `doubleJumpHeight`: 2 m
* `hardLandingCooldown`: 0.75 s


> [!NOTE]
> **💡 Dica**: Enquanto plana, as forças contínuas aplicadas pela scene ficam 1,5 vezes mais fortes, e forças ascendentes podem levantar o player. Veja [Forças ao planar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/player-physics/#forces-while-gliding).



> [!NOTE]
> **💡 Dica**: Nenhuma dessas propriedades pode ser menor que 0. Se você definir uma delas com um valor negativo, ele será limitado a 0. Definir esses valores como zero terá o mesmo efeito que usar o `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` para bloquear o uso de certas teclas.
> 
> Você só pode afetar a locomoção do player se ele estiver dentro dos limites da scene. Para afetar os avatares de outros players, você precisa executar o código que afeta a locomoção deles na própria instância deles.


Você pode criar um [smart wearable](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables/) que faz o player sempre correr mais rápido ou saltar mais alto. Se tanto a scene quanto um smart wearable definirem valores diferentes para esses parâmetros, os valores da scene são sempre usados.

Para garantir que ninguém tenha vantagem injusta em uma scene de parkour, você pode impor os parâmetros padrão adicionando explicitamente seus valores padrão na sua scene:

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	walkSpeed: 1.5,
	jogSpeed: 8 ,
	jumpHeight: 1,
	runJumpHeight: 1.5,
	hardLandingCooldown: 0.75
})
```

## Restringir a locomoção

Você pode restringir quais ações o player pode executar na sua scene. Use isso para congelar o player ou para restringir formas específicas de locomoção, por exemplo, para impedir que o player salte ou corra.

### Congelar o player

Você pode congelar o player para que nenhuma das teclas de input possa mover o avatar. Isso pode ser útil para muitas mecânicas de jogo. Também é uma boa prática congelar um player enquanto executa uma animação importante que não deveria ser interrompida pelo movimento, ou enquanto uma [Virtual Camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/) aponta para longe do avatar e você não quer que o player se mova às cegas.

Use o `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` component no `engine.PlayerEntity` para impedir que os inputs do player afetem a locomoção do avatar. O avatar permanecerá parado, e o player só poderá girar a câmera.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: true,
	}),
})
```

Tenha em mente as seguintes considerações:

* Enquanto as interações do player estão desativadas, o avatar ainda é afetado por forças externas, como gravidade ou plataformas em movimento.
* A `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` component só pode ser usado com a `engine.PlayerEntity` entity. Ele só pode afetar o player atual, não pode afetar outros players.
* Este component só afeta o player enquanto o avatar estiver dentro dos limites da sua scene. Sua locomoção deixa de ser restringida assim que ele sai da scene.
* Enquanto as interações do player estão desativadas, o player não pode fazer emotes livremente, mas a scene pode acionar animações no avatar.
* Os inputs do player não afetam o avatar, mas os [eventos globais de input](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/#global-input-events) ainda podem ser escutados pela scene. Você poderia usá-los para controlar um veículo, ou usar um [Virtual Camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/) para seguir outra entity enquanto ela se move, tratando-a como um avatar alternativo.

### Restringindo tipos específicos de locomoção

Em vez de congelar o player por completo, você pode restringir certas formas específicas de locomoção do player. Isso pode ser usado por razões de gameplay, por exemplo, para preservar a dificuldade de um platformer impedindo o double-jump e o glide. Essas habilidades poderiam até ser alternadas dinamicamente como uma mecânica de jogo, por exemplo, dando ao player uma barra de stamina e impedindo que ele corra quando ela se esgotar. Isso também pode ser usado para definir o tom de uma scene, por exemplo, impedindo correr ou saltar em um local que deve ser sereno. O `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` inclui as seguintes opções:

* `disableWalk`: O player não pode andar lentamente (pressionando control). Se o player tentar andar, ele vai trotar ou correr em vez disso, se permitido.
* `disableRun`: O player não pode correr (pressionando shift). Se o player tentar correr, ele vai trotar em vez disso, se permitido.
* `disableJog`: O player não pode trotar (esta é a velocidade de movimento padrão). Se o player tentar trotar, ele vai correr ou andar em vez disso, se permitido.
* `disableJump`: O player não pode saltar.
* `disableEmote`: O player não pode fazer emotes voluntariamente. A scene pode acionar animações no avatar do player.
* `disableDoubleJump`: O player não pode fazer um double-jump.
* `disableGliding`: O player não pode planar.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: false,
		disableWalk: false,
		disableRun: true,
		disableJog: true,
		disableJump: true,
		disableEmote: true,
		disableDoubleJump: true,
		disableGliding: true
	}),
})
```

### Sintaxe avançada

Para usar o component sem nenhum helper, você pode usar a seguinte sintaxe:

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.createOrReplace(engine.PlayerEntity, {
	mode: {
		$case: 'standard',
		standard: {
			disableAll: false,
			disableWalk: false,
			disableRun: true,
			disableJog: true,
			disableJump: true,
			disableEmote: true,
		},
	},
})
```


> [!NOTE]
> **💡 Dica**: Para um exemplo funcional, veja a [`0,1-input-modifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier) scene de teste, que alterna cada `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` flag de movimento separadamente e mostra um avatar ainda sendo carregado por uma plataforma em movimento enquanto o movimento está bloqueado.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
