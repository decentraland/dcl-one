> Saiba como lidar com cliques de usuário na sua scene.

# Registrar Callback

A maneira mais fácil de lidar com eventos de botão é registrar uma função de callback para uma entity específica. Sempre que essa entity for interagida usando um botão específico, a função de callback é chamada.

Se você precisar adicionar o mesmo comportamento a múltiplas entities semelhantes, considere usar a [baseada em System](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/) em vez de adicionar callbacks a cada entity. A abordagem baseada em System pode resultar em mais eficiência, pois você itera sobre uma lista de entities semelhantes.

A abordagem de Register callback é especialmente útil se você quiser descrever um comportamento que afeta uma única entity, pois é mais direta.


> [!WARNING]
> **📔 Nota**:\
> Para que uma entity seja interativa, ela **devem** deve ter um [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/). Veja [obstáculos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#obstacles) para mais detalhes.


## Pointer down

Use `pointerEventsSystem.onPointerDown()` para detectar pressionamentos de um botão específico.

Esta instrução requer dois parâmetros:

* `dados`: Um objeto que contém o seguinte:
  * `entity`: A entity a ser tratada
  * `opts`: Um objeto com dados adicionais opcionais:
    * `button`: Qual botão escutar. Veja [Pointer buttons](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções compatíveis. Se nenhum botão for especificado, então todos os botões serão escutados, incluindo botões de movimento como avançar e pular.
    * `maxDistance`: A distância máxima entre a entity e o **avatar**, em metros. 10 por padrão. Veja [Limites de distância](#distance-limits).
    * `maxCameraDistance`: A distância máxima entre a entity e a **câmera ativa**, em metros. Não definido por padrão. Veja [Limites de distância](#distance-limits).
    * `hoverText`: Qual string exibir na dica de feedback ao passar o cursor. "Interact" por padrão.
    * `showFeedback`: Se false, oculta tanto a dica de passar o cursor quanto o destaque de borda para esta entity. *true* por padrão.
    * `showHighlight`: Se true, os players verão o destaque de borda ao passar o cursor sobre a entity. *true* por padrão. Este valor só é considerado se `showFeedback` está *true*.
* `cb`: Uma função de callback para executar a cada vez que ocorre um evento de button down enquanto aponta para a entity

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Clique' },
	},
	function () {
		console.log('entity clicada')
	}
)
```

O comando acima deixa a função de callback registrada, e ela será chamada toda vez que o evento de botão relacionado ocorrer. Observe que a callback deve ser uma função síncrona; `async` funções não são suportadas aqui e resultam em um erro.


> [!WARNING]
> **📔 Nota**:\
> Somente um `pointerEventsSystem.onPointerDown` pode ser registrado por entity. Depois de adicionado, ele continuará ouvindo eventos até que o listener seja removido. Não execute isso recorrentemente dentro de um System, pois isso continuaria sobrescrevendo o comportamento do pointer event.


## Feedback ao passar o cursor

É muito importante dar aos players algum tipo de indicação de que uma entity pode ser interagida.

Ao registrar uma input action com o `EventsSystem`, por padrão os players verão:

* Um destaque de borda na entity
* Uma dica de hover perto do cursor com um ícone do botão que precisam pressionar e uma string que diz "Interact".

Esses elementos podem ser ativados/desativados e personalizados.

O feedback de hover na UI exibe um ícone diferente dependendo da input que você selecionar no `button` campo. No PC, ele exibe um ícone com um `E` para `InputAction.IA_PRIMARY`, um `F` para `InputAction.IA_SECONDARY`, e um mouse para `InputAction.IA_POINTER`.

Altere a string alterando o `hoverText` valor. Mantenha essa string curta, para que seja rápida de ler e não seja muito intrusiva na tela.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Abrir porta' },
	},
	function () {
		// abrir door
	}
)
```

Para ocultar a dica de hover, mas deixar o destaque de borda, defina o valor de `hoverText` para "".

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, hoverText: ''}},
  function () {
    console.log("clicado no item interativo surpresa")
  }
)
```

Para ocultar o destaque de borda, mas deixar a dica de hover, defina `showHighlight` como *false*.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: {
			button: InputAction.IA_PRIMARY,
			hoverText: 'Abrir porta',
			showHighlight: false,
		},
	},
	function () {
		console.log('porta secreta aberta')
	}
)
```

Para ocultar tanto a dica de hover quanto o destaque de borda, defina `showFeedback` como *false*. Ao fazer isso, o cursor não mostra nenhum ícone, texto ou destaque de borda.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, showFeedback: false}},
  function () {
    console.log("porta secreta aberta")
  }
)
```

### Alterar feedback existente

Ao registrar uma input action com o `EventsSystem`, isso está criando um `PointerEvents` component e adicionando-o à entity interativa nos bastidores. Este component lida com o comportamento da dica de hover da UI. Para alterar o comportamento do feedback de hover, modifique este component. Veja [Mostrar feedback](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/#show-feedback) para saber mais sobre como lidar com este component.

```ts
const hoverFeedback = PointerEvents.getMutable(myEntity)

if (hoverFeedback.pointerEvents[0]?.eventInfo) {
	hoverFeedback.pointerEvents[0].eventInfo.hoverText = 'Fechar porta'
}
```

## Limites de distância

Por padrão, um player só pode interagir com uma entity quando seu **avatar** está a até 10 metros dela. Se o player estiver longe demais, o efeito de destaque da entity fica vermelho em vez de verde, e os pointer events não são disparados.

Use `maxDistance` para alterar esse alcance:

```ts
// Apenas clicável quando o avatar do player estiver a até 5 metros
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 5 } },
	function () {
		console.log('entity clicada')
	}
)
```

### Limitar pela distância da câmera

`maxCameraDistance` mede a partir da **câmera atualmente ativa** em vez do avatar. Isso é útil quando sua scene usa uma [Virtual Camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/#using-virtual-cameras) colocada longe do player, e você quer que a entity possa ser clicada com base no que a câmera consegue ver.

```ts
// Apenas clicável quando a câmera ativa estiver a até 15 metros
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxCameraDistance: 15 } },
	function () {
		console.log('entity clicada')
	}
)
```

### Como os dois limites se combinam

As duas opções são verificações independentes. Quais delas são executadas depende do que você definir:

| O que você definiu         | O que o player precisa                       |
| -------------------------- | -------------------------------------------- |
| Nenhum                     | Avatar a até 10 metros (o padrão)            |
| `maxDistance` apenas       | Avatar a até `maxDistance`                   |
| `maxCameraDistance` apenas | Câmera a até `maxCameraDistance`             |
| Ambos                      | **Qualquer um** basta uma verificação passar |

Definir ambos é uma forma de dizer "perto o suficiente para ir até ele, **ou** perto o suficiente para vê-lo claramente":

```ts
// Clicável quando o avatar estiver a até 3 metros OU a câmera estiver a até 20 metros
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 3, maxCameraDistance: 20 } },
	function () {
		console.log('entity clicada')
	}
)
```


> [!WARNING]
> **📔 Nota**: `maxCameraDistance` requer `@dcl/sdk` versão 7.28.0 ou mais recente. O suporte para isso já está disponível no explorer baseado em Bevy. Se sua scene precisar se comportar de forma idêntica em todos os clients hoje, confie em `maxDistance` e trate `maxCameraDistance` como uma melhoria.


### maxPlayerDistance está obsoleto

`maxPlayerDistance` é um alias obsoleto de `maxDistance`: ambos medem a distância a partir do avatar. Use `maxDistance` em novas scenes.

Se uma scene definir ambos, **o maior** dos dois é usado como o limite de distância do avatar.

## Pointer up

Use `pointerEventsSystem.onPointerUp` para registrar uma função de callback que será chamada quando o player indicado soltar o botão enquanto aponta para a entity.

```ts
pointerEventsSystem.onPointerUp(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Botão solto' },
	},
	function () {
		console.log('botão solto')
	}
)
```

Esta instrução requer dois parâmetros:

* `dados`: Um objeto que contém o seguinte:
  * `entity`: A entity a ser tratada
  * `opts`: Um objeto com dados adicionais opcionais:
    * `button`: Qual botão escutar. Veja [Pointer buttons](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções compatíveis. Se nenhum botão for especificado, então todos os botões serão escutados, incluindo botões de movimento como avançar e pular.
    * `hoverText`: Qual string exibir na dica de feedback ao passar o cursor. "Interact" por padrão.
    * `showFeedback`: Se false, oculta a dica de hover para esta entity. *true* por padrão.
    * `maxDistance`: Quão longe o avatar do player pode estar da entity e ainda interagir com ela, em metros. 10 por padrão. Se o player estiver longe demais, não haverá feedback de hover e os pointer events não funcionarão. Veja [Limites de distância](#distance-limits).
* `cb`: Uma função de callback para executar a cada vez que ocorre um evento de button up enquanto aponta para a entity.

A mesma entity pode ter duas callbacks diferentes registradas, uma para `pointerEventsSystem.onPointerDown` e uma para `pointerEventsSystem.onPointerUp`. A entity só pode registrar uma callback de cada, [Lidar com múltiplos botões](#handle-multiple-buttons) para detectar botões diferentes em uma mesma callback.


> [!WARNING]
> **📔 Nota**: O feedback de hover para um evento de button up só é exibido quando o botão está atualmente pressionado. Se o player apontar para a entity sem manter o botão pressionado, ele não verá nenhum feedback, ou verá o feedback do evento de button down, se houver.


## Entrada e saída do hover

Use `pointerEventsSystem.onPointerHoverEnter` para executar uma callback quando o cursor do player começar a apontar para uma entity, e `pointerEventsSystem.onPointerHoverLeave` quando o cursor parar de apontar para ela.

```ts
pointerEventsSystem.onPointerHoverEnter(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('Cursor começou a passar sobre a entity')
    }
)

pointerEventsSystem.onPointerHoverLeave(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('Cursor parou de passar sobre a entity')
    }
)
```

Essas callbacks são úteis para efeitos personalizados de hover, como tocar um som ou animar a entity quando o player mira nela.

## Remover callbacks

Para remover uma função de callback, use a função remove correspondente:

```ts
pointerEventsSystem.removeOnPointerDown(myEntity)
pointerEventsSystem.removeOnPointerUp(myEntity)
pointerEventsSystem.removeOnPointerHoverEnter(myEntity)
pointerEventsSystem.removeOnPointerHoverLeave(myEntity)
```

Depois de removido, o feedback de hover na entity não deverá mais ser exibido, e a entity não deverá mais ser interativa.

Cada função remove remove apenas a entrada que sua função register correspondente adicionou. Registrar e remover repetidamente a mesma callback não deixa entradas antigas na entity.

## Interações por proximidade

Para interações que são disparadas com base na proximidade do player e não na mira do cursor, veja [**Proximity Events**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events/). O `pointerEventsSystem` inclui helpers equivalentes (`onProximityDown`, `onProximityUp`, `onProximityEnter`, e `onProximityLeave`) que seguem o mesmo padrão das funções descritas nesta página.

Uma entity pode conter tanto um handler de cursor quanto um handler de proximidade para o mesmo tipo de evento. Ambas as callbacks são executadas, cada uma quando sua própria condição é atendida.

### Dados da input action

Busque dados de uma input action, como o botão que foi pressionado, a entity que foi atingida, a direção e o comprimento do ray etc. Veja ([Veja a documentação](https://github.com/decentraland/docs/tree/main/README.md)) para uma descrição de todos os dados disponíveis.

Para buscar esses dados, passe um parâmetro para a função de callback. Esse parâmetro contém a estrutura de dados completa com dados sobre o input event.

```ts
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { button: InputAction.IA_PRIMARY } },
	function (cmd) {
		console.log(cmd.hit?.entityId)
	}
)
```

### Lidar com múltiplos botões

Você não pode registrar mais de um `onPointerDown` em uma única entity. Idealmente, você deve usar a [baseada em System](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/) abordagem, pois isso permite lidar com quantas entradas diferentes você quiser e exibir uma dica de feedback de hover na UI para cada botão.

Como alternativa, você pode usar a abordagem de Register callback e definir o `button` campo como `InputAction.IA_ANY`.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_ANY}},
  function (cmd) {
      if(cmd.button === InputAction.IA_POINTER){
        // fazer X
      } else if (cmd.button === InputAction.IA_PRIMARY){
        // fazer Y
      }
  }
)
```

Essa abordagem não é ideal, pois a dica de hover mostra uma única string e não especificará qual ação ativar. Observe que isso fará a função de callback executar para cada input action, inclusive as teclas de movimento, então você deve filtrar apenas as ações com as quais se importa.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
