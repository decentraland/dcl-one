> Configure os controles nativos de toque na tela para a sua scene.

# Controles na Tela

No cliente mobile, os jogadores interagem com sua scene por meio de um conjunto de controles nativos na tela — um joystick virtual, um retículo e um gamepad de botões. O `TouchScreenControls` component permite que sua scene redefina esse HUD: simplifique-o, oculte o joystick ou o retículo, altere o que o botão central grande faz, troque o glifo de um button pelo seu próprio ícone ou oculte buttons por completo e substitua-os pela sua própria UI.

<figure><img src="/docs/_remote/8414c098957fe0e8df7af46b.webp" alt="The default mobile HUD: virtual joystick on the left, gamepad buttons on the right"><figcaption><p>Os controles padrão na tela, antes de qualquer personalização</p></figcaption></figure>


> [!NOTE]
> O component é aplicado automaticamente enquanto o player estiver dentro da sua scene e volta aos padrões no momento em que ele sair, então scenes que não o usam não são afetadas. Ele só afeta plataformas com controles nativos na tela: não faz nada no desktop e não tem efeito em VR.


## O que aparece por padrão

Do SDK `7.32.0`, toda scene começa com três buttons do gamepad visíveis:

* `IA_JUMP`, o botão central grande
* `IA_POINTER`, o botão de interação
* `IA_PRIMARY`, o botão E

`IA_SECONDARY` e `IA_ACTION_3` a `IA_ACTION_6` começam ocultos. Você só precisa `TouchScreenControls` quando quiser algo diferente disso.


> [!WARNING]
> **📔 Nota**: O padrão de três buttons e o `showDefault()` precisa do SDK7 `7.32.0` ou posterior. Em versões anteriores, todos os oito buttons aparecem, a menos que sua scene oculte alguns, e `showDefault()` não existe, então `showAll()` é a única maneira de trazer os buttons de volta.


O SDK escreve esse conjunto padrão uma vez no início da scene, pouco antes do primeiro frame, e o ignora quando sua scene já escolheu seus próprios buttons. Sua scene conta como tendo escolhido se qualquer um dos casos abaixo for verdadeiro:

* Uma das funções auxiliares de visibilidade de buttons já foi executada: `hideAll()`, `showAll()`, `showDefault()` ou `hide()`.
* O component na RootEntity já lista pelo menos um button, por exemplo porque sua scene o escreveu com `createOrReplace` no nível superior de `index.ts`, fora de `main()`.

`hideJoystick()`, `showJoystick()`, `hideCrosshair()`, `showCrosshair()` e `setMainAction()` não contam: o conjunto padrão de buttons ainda se aplica junto com eles.

Sua `main()` função é executada depois dessa gravação, então tudo o que você fizer ali fica sobre o conjunto padrão. Uma função auxiliar como `hide()` faz merge com ele, enquanto `createOrReplace` substitui a lista inteira pela sua.

Para trazer os buttons ocultos de volta, chame `showAll()` para todos os oito. `showDefault()` restaura o conjunto padrão e mantém seus ícones, joystick, crosshair e main action inalterados.

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// Mostre todos os buttons na tela, incluindo os buttons 1 / 2 / 3 / 4
TouchScreenControls.showAll()
```

## Como o layout dos buttons funciona

Os buttons do gamepad formam uma única **pilha de prioridade**. A ordem é fixa:

1. `IA_JUMP`
2. `IA_POINTER`
3. `IA_PRIMARY` (E)
4. `IA_SECONDARY` (F)
5. `IA_ACTION_3` (1)
6. `IA_ACTION_4` (2)
7. `IA_ACTION_5` (3)
8. `IA_ACTION_6` (4)

As posições na tela também são fixas. Os **visíveis** buttons preenchem essas posições do topo da pilha para baixo — então o que você altera é *quais* buttons ficam visíveis e *qual deles lidera*, não a ordem deles.

| Quando você…                                            | Os controls…                                                                                                                                |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| **Ocultar um button** (qualquer button, inclusive pulo) | Cada button de menor prioridade sobe para preencher a lacuna. Oculte pulo e `IA_POINTER` vai para o lugar central.                          |
| **Não altere nada**                                     | O conjunto padrão se aplica: `IA_JUMP` é o botão central grande, com `IA_POINTER` e `IA_PRIMARY` nos slots ao redor dele.                   |
| **Defina um botão principal** com `mainAction`          | Essa action vai para a frente e se torna o botão central; todos os outros buttons mantêm sua ordem normal.                                  |
| **Defina um botão principal que também esteja oculto**  | Ocultar prevalece — o button continua oculto.                                                                                               |
| **Deixe 5 buttons ou menos visíveis**                   | Todos eles aparecem diretamente (o botão central mais até quatro ao redor dele); não há menu "+".                                           |
| **Deixe mais de 5 buttons visíveis**                    | O "+" ocupa o último slot, então quatro aparecem diretamente (o botão central mais três) e o restante fica atrás do toggle de overflow "+". |

<figure><img src="/docs/_remote/ac9947bc2b985676d54e81a8.webp" alt="Three HUDs showing the gamepad reflowing as the number of visible buttons changes"><figcaption><p>Como os buttons se reorganizam com a contagem de visíveis. <strong>Esquerda (7 buttons):</strong> o botão principal, um arco de 1–3 e o "+" segurando o overflow (4–7) em uma coluna que sobe para cima. <strong>Centro (5 buttons):</strong> o mesmo arco, com uma coluna de overflow mais curta (4–5) atrás do "+". <strong>Direita (4 buttons):</strong> todos os quatro aparecem diretamente e o "+" desaparece.</p></figcaption></figure>


> [!NOTE]
> O `1`/`2`/`3`/`4` buttons começam ocultos. `showAll()` os traz de volta, mas isso deixa oito buttons visíveis, então os de menor prioridade acabam atrás do "+". Para colocar um button numerado diretamente na tela, mantenha a contagem de visíveis em cinco ou menos: mostre apenas os buttons que sua scene realmente usa.


## Tarefas comuns

`TouchScreenControls` inclui um conjunto de funções auxiliares práticas. Cada uma escreve o component na `RootEntity` (onde o client o lê) e faz merge com o valor atual, então você pode chamá-los de qualquer lugar.

**Alterar o botão principal** — faça o botão central grande disparar uma action diferente:

```ts
import { TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.setMainAction(InputAction.IA_PRIMARY)
}
```

**Ocultar o joystick ou o retículo** — remova o stick de movimento e/ou o retículo de mira e traga-os de volta com seus `mostrar` equivalentes:

```ts
TouchScreenControls.hideJoystick()
TouchScreenControls.hideCrosshair()

// e para trazê-los de volta:
TouchScreenControls.showJoystick()
TouchScreenControls.showCrosshair()
```

**Ocultar buttons específicos** — passe as actions que quer remover (o restante sobe em cascata):

```ts
TouchScreenControls.hide([InputAction.IA_SECONDARY, InputAction.IA_JUMP])
```

**Ocultar todos os buttons, mostrar todos os buttons ou voltar ao conjunto padrão:**

```ts
TouchScreenControls.hideAll()     // nenhum button do gamepad
TouchScreenControls.showAll()     // todos os oito buttons
TouchScreenControls.showDefault() // jump, pointer e primary apenas
```

Esses três só afetam os buttons do gamepad. Eles não restauram um joystick ou retículo oculto e não alteram seus ícones personalizados nem sua main action. Use `showJoystick()` / `showCrosshair()` para isso.

**Substituir o ícone de um button** — para controle total (ícones personalizados, várias alterações ao mesmo tempo), escreva o component bruto em `engine.RootEntity`:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideCrosshair: true,
		mainAction: InputAction.IA_PRIMARY,
		touchInputs: [
			{
				inputAction: InputAction.IA_PRIMARY,
				icon: { tex: { $case: 'texture', texture: { src: 'images/grab.png' } } },
			},
		],
	})
}
```

As funções auxiliares em resumo:

| Função auxiliar         | O que ele faz                                                                                                                                                    |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `setMainAction(action)` | Define qual action o botão central grande dispara.                                                                                                               |
| `hideJoystick()`        | Oculta o joystick virtual nativo.                                                                                                                                |
| `showJoystick()`        | Mostra o joystick virtual nativo novamente.                                                                                                                      |
| `hideCrosshair()`       | Oculta a crosshair / retículo na tela.                                                                                                                           |
| `showCrosshair()`       | Mostra a crosshair / retículo na tela novamente.                                                                                                                 |
| `hide(actions)`         | Oculta os buttons do gamepad fornecidos (mesclado na configuração atual).                                                                                        |
| `hideAll()`             | Oculta todos os buttons do gamepad.                                                                                                                              |
| `showAll()`             | Mostra todos os oito buttons do gamepad (limpa a lista de buttons ocultos). Não **não** afeta o joystick/crosshair.                                              |
| `showDefault()`         | Restaura o conjunto padrão: jump, pointer e primary visíveis, secondary e os actions 3 a 6 ocultos. Mantém ícones, joystick, crosshair e main action como estão. |

## Propriedades

Escreva estas diretamente quando usar `createOrReplace`:

| Propriedade     | Tipo                                                                                                                     | Descrição                                                                                                                                                                                                                                                                                                               |
| --------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hideJoystick`  | *boolean*                                                                                                                | Oculta o joystick virtual nativo de movimento.                                                                                                                                                                                                                                                                          |
| `hideCrosshair` | *boolean*                                                                                                                | Oculta a crosshair / retículo na tela.                                                                                                                                                                                                                                                                                  |
| `mainAction`    | [*InputAction*](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) | Move essa action para a frente da stack, tornando-a o botão central grande; os outros buttons mantêm sua ordem. Só actions de gamepad são válidas (veja abaixo). Quando não definido, o primeiro button visível (`IA_JUMP` por padrão) lidera. Veja [Como o layout dos buttons funciona](#how-the-button-layout-works). |
| `touchInputs`   | *array*                                                                                                                  | Substituições por button. Escrever esse campo substitui a lista inteira, então qualquer button que você omitir é mostrado com seu glifo padrão, inclusive os que começam ocultos.                                                                                                                                       |

Cada `touchInputs` a entrada tem:

| Campo         | Tipo                                                                                                                     | Descrição                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `inputAction` | [*InputAction*](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) | Qual button na tela esta entrada configura.                                                                                                                                                                                                                                                                                                                                                                                                               |
| `hide`        | *boolean*                                                                                                                | Oculta este button. O padrão é `false` (mostrado). Qualquer button pode ser ocultado, **incluindo `IA_JUMP`** — o restante sobe em cascata para ocupar seu lugar.                                                                                                                                                                                                                                                                                         |
| `icon`        | [*TextureUnion*](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_background/#background) (opcional)                   | Substitui o glifo do button por uma imagem da scene. Use a variante `texture` com um `src` mapeado para conteúdo `{ tex: { $case: 'texture', texture: { src: 'images/grab.png' } } }`. Só caminhos de conteúdo da scene são suportados (não URLs externas, avatar ou texturas de vídeo). Para o botão jump, isso substitui todos os seus estados dinâmicos (jump / double-jump / glide). Se o caminho não puder ser resolvido, o glifo integrado é usado. |

## Quais actions mapeiam para quais buttons

O [`InputAction`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) os valores aqui são os mesmos usados em [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/) e [Eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/). Estas são as actions que mapeiam para os buttons na tela:

| InputAction                                                   | Button na tela                  |
| ------------------------------------------------------------- | ------------------------------- |
| `IA_JUMP`                                                     | O botão central grande (padrão) |
| `IA_POINTER`                                                  | O botão de interação            |
| `IA_PRIMARY`                                                  | O botão E                       |
| `IA_SECONDARY`                                                | O botão F                       |
| `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` | Os buttons 1 / 2 / 3 / 4        |


> [!WARNING]
> `IA_ANY` e `IA_MODIFIER` são valores meta — não mapeiam para um button e não podem ser usados aqui.


## Exemplo

Para ocultar o joystick, mantenha ocultos os buttons F e numerados e dê ao botão central de jump um ícone personalizado incluído na sua scene:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideJoystick: true,
		touchInputs: [
			// Esta lista substitui a padrão, então os buttons que normalmente começam
			// ocultos também precisam ser listados aqui
			{ inputAction: InputAction.IA_SECONDARY, hide: true },
			{ inputAction: InputAction.IA_ACTION_3, hide: true },
			{ inputAction: InputAction.IA_ACTION_4, hide: true },
			{ inputAction: InputAction.IA_ACTION_5, hide: true },
			{ inputAction: InputAction.IA_ACTION_6, hide: true },
			{
				inputAction: InputAction.IA_JUMP,
				icon: { tex: { $case: 'texture', texture: { src: 'images/banana.png' } } },
			},
		],
	})
}
```

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="A mobile HUD with the joystick hidden and the central jump button showing a custom banana icon"><figcaption><p>O resultado: joystick removido, buttons numerados ocultos (então o "+" desaparece) e o botão de jump com novo ícone</p></figcaption></figure>

Para substituir completamente os controls nativos, oculte-os aqui e construa seus próprios touch buttons com [Vinculação de entrada da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_input_binding/).

## Relacionado

* [Vinculação de entrada da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_input_binding/)
* [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/)
* [Eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/)
* [Detectar a plataforma pelo código](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
