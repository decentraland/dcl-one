> Referencie itens no seu código por nome ou por tag.

# Referenciar itens no código

Você pode referenciar itens adicionados através da interface de arrastar e soltar do Creator Hub no seu código. Isto é útil para adicionar comportamento sofisticado a esses itens, ou para os referenciar a partir de outras partes do seu código.

## Obter por nome

Ao usar o Creator Hub e adicionar entities arrastando-as para o canvas, cada entity tem um nome único. Use a `engine.getEntityOrNullByName()` function para referenciar uma dessas entities a partir do seu código.

Use o seletor `EntityNames` enum para aceder facilmente aos nomes das entities que adicionou via o Creator Hub, ou escreva o nome como uma string tal como aparece na entity tree da scene no Scene Editor.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	// Use o enum EntityNames
	const door1 = engine.getEntityOrNullByName(EntityNames.Door_1)

	// Escreva o nome como uma string
	const door2 = engine.getEntityOrNullByName('Door 2')

	// Garanta que ambas as doors existem na scene
	if (door1 && door2) {
		// 
	}

}
```

![](/docs/_remote/6c4b9156f30308b7eb4c3fc9.webp)

O `EntityNames` enum contém a lista completa de entities adicionadas pelo Creator Hub e é atualizado imediatamente assim que faz quaisquer alterações. Se importar `EntityNames.` para o seu código, o seu IDE apresentará um dropdown com todos os nomes das entities disponíveis.

![](/docs/_remote/c76350566b1b5677df83aaca.webp)

Também pode usar a `engine.getEntityByName<EntityNames>()` function, passando `<EntityNames>` como um [TypeScript generic](https://www.typescriptlang.org/docs/handbook/2/generics.html), para validar que uma entity com esse nome realmente existe na sua scene. Se a entity referenciada for renomeada no Creator Hub, este método avisá-lo-á com um erro. Como a saída desta function não pode ser `null`, pode evitar verificar se a entity existe.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	const door1 = engine.getEntityByName<EntityNames>(EntityNames.Door_1)

	// Não é necessário verificar null
	console.log(Transform.get(door1).position.x)

}
```


> [!WARNING]
> **📔 Nota**: Certifique-se de que só usa `engine.getEntityOrNullByName()` e `engine.getEntityByName()` dentro da `main()` function, em functions que executam depois de `main()`, ou num system. Se for usado fora de um desses contextos, as entities criadas no Scene Editor podem ainda não estar instanciadas.


Depois de obter uma referência para uma entity com qualquer um dos métodos acima, tem liberdade para executar qualquer ação com ela, como adicionar ou remover components, modificar valores de components existentes, ou até remover a entity do engine.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// obter entity
	const door = engine.getEntityOrNullByName(EntityNames.Door_3)
	// verificar se a entity existe
	if (door) {
		// adicionar um callback de pointer events
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open' },
			},
			function () {
				// abrir door
			}
		)
	}
}
```

Todas as entities adicionadas via o Scene Editor têm um `Nome` component, também pode iterar sobre todas elas assim:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## Obter por Tag

Também pode obter entities pelas suas tags. Tags são uma forma de agrupar entities e são úteis para identificar entities que têm o mesmo propósito ou comportamento.

Adicione Tags a uma entity através da **Tags** secção no topo do properties panel do item. Pode escolher entre tags genéricas como **Tag Group 1** até **Tag Group 4**, ou criar as suas próprias com um nome mais específico.

![](/docs/_remote/feb4023234754c8ebd2b7590.webp)


> [!NOTE]
> **💡 Dica**: Uma única entity pode ter várias tags atribuídas.
> 
> <img src="/docs/_remote/22e3f2ff154ade672d73545c.webp" alt="" data-size="original">


Pode então obter todas as entities que tenham uma tag específica usando a `engine.getEntitiesByTag()` function. Isto é ideal quando pretende iterar sobre um grupo de entities com o mesmo propósito ou comportamento.

```ts
import { engine } from '@dcl/sdk/ecs'

export function main() {
	const taggedEntities = engine.getEntitiesByTag('myTag')
  
	for (const entity of taggedEntities) {
      // Faça algo com cada entity
    }
}
```

Também pode adicionar ou remover tags a uma entity a partir do seu código. Isto é útil se quiser alterar tags com base em alguma lógica, ou spawn entities dinamicamente que tenham tags específicas.

```ts
import { Tags } from '@dcl/sdk/ecs'

Tags.remove(entity, tagName);
Tags.add(entity, tagName);
```

## Obter todos os children de um item

Depois de ter uma referência a um item específico, pode obter todos os items que estão agrupados como seus children na entity tree à esquerda do ecrã. O script seguinte obtém a parent entity e depois itera sobre cada um dos seus children que tenham um componente Transform. Pode então aplicar qualquer lógica personalizada que quiser à medida que itera sobre cada um.

```ts
import { engine, Entity, Transform, Name, getEntitiesWithParent } from '@dcl/sdk/ecs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// obter entity pai
	const parent = engine.getEntityByName<EntityNames>(EntityNames.ParentEntity)

	// obter a lista completa de children
	const children = getEntitiesWithParent(engine, parent)
	for (const child of children) {
   		// processar cada child entity
	}
}
```

## Triggers de smart item

Pode detetar os **eventos Trigger**, e responder a estes com código personalizado. Por exemplo, poderia colocar um smart item button e ativar código personalizado quando o botão for clicado.

Use `getTriggerEvents` para obter um objecto que pode tratar trigger events de um smart item específico e depois usar a `.on()` function do objeto devolvido para subscrever uma função callback. Esta função callback é executada sempre que o trigger event acontece.

Por exemplo, se uma scene tiver um button com o seguinte generic **On Click** event, pode escrever o código abaixo para executar código personalizado sempre que o botão for ativado.

![](/docs/_remote/ed2a86256c21aaf6084b2d47.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const restart = engine.getEntityOrNullByName(EntityNames.Restart_Button)
	if (restart) {
		const restart_event = getTriggerEvents(restart)
		restart_event.on(TriggerType.ON_CLICK, () => {
			// restartGame()
		})
	}
}
```

Também pode subscrever-se de forma semelhante a qualquer outro tipo de trigger events, como **ON\_PLAYER\_ENTERS\_AREA**, **ON\_SPAWN**, **ON\_TWEEN\_END**, etc.

## Actions de smart item

Pode detetar a ativação dos **Actions**, e responder a estes com código personalizado. Por exemplo, poderia colocar um smart item door e executar código personalizado sempre que a **Open** action for chamada.

Use `getActionEvents` para obter um objeto para tratar as actions de um smart item específico. Depois pode usar a `.on()` function do objeto devolvido para subscrever uma função callback. Esta função callback é executada sempre que a action acontece, independentemente de a action ter sido ativada por outro smart item ou até pelo seu próprio código personalizado.

Por exemplo, se uma scene tiver uma door com a seguinte default **Open** action, pode escrever o código abaixo para executar código personalizado sempre que a door for aberta.

![](/docs/_remote/d8d14bd55320cbd4d130d66f.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (door) {
		// detetar actions
		const actions = getActionEvents(door)
		actions.on('Open', () => {
			console.log('Porta aberta!!')
			// código personalizado
		})

		// detetar triggers
		const triggers = getTriggerEvents(door)
		triggers.on(TriggerType.ON_CLICK, () => {
			console.log('Porta clicada!!')
			// código personalizado
		})
	}
}
```

Também pode emitir action events a partir do seu código, isto permite-lhe tirar partido de actions que já estão definidas dentro do componente Action do smart item. O seguinte snippet chama a action "Open" num smart item door sempre que um smart item button é acionado.

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const button = engine.getEntityOrNullByName(EntityNames.Red_Button)
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (button && door) {
		// referências a actions e triggers
		const buttonTriggers = getTriggerEvents(button)
		const doorActions = getActionEvents(door)

		// detetar triggers no button
		buttonTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
			// abrir door
			doorActions.emit('Open', {})
		})
	}
}
```


> [!NOTE]
> **💡 Dica**: Se não estiver a tentar fazer algo muito complicado, em vez de escrever código também pode criar um smart item personalizado para tratar das actions que pretende executar. Veja [Tornando qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).


## Outros componentes de smart item

Smart items podem incluir componentes especiais que fazem parte da library asset-packs, como `States` ou `Counter`. Estes components não fazem parte do Decentraland SDK, mas podem ser obtidos através da `getComponents()` function da library. Depois pode ler ou escrever valores nestes components a partir do código da sua scene, para ter uma integração ainda mais estreita entre o comportamento do smart item e o código.

O exemplo abaixo lê e regista o valor de um component State de um smart item chest sempre que as actions do chest são acionadas.

```ts

import { engine } from '@dcl/sdk/ecs'
import { getComponents } from '@dcl/asset-packs'
import { getTriggerEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'


export function main() {

    const chest = engine.getEntityByName<EntityNames>(EntityNames.chest)
 
    if (chest) {

        const chestTriggers = getTriggerEvents(chest)

        chestTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
            const { States } = getComponents(engine)
            let state = States.getMutableOrNull(chest)?.currentValue
            console.log( "novo estado do chest ", state)
        })
    }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/expandir-com-codigo/reference-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
