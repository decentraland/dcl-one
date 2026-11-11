> Detecte alterações em um component e execute funções a cada alteração

# Inscrever-se nas Alterações

Uma forma elegante de escrever o seu código é subscrever eventos e executar uma função sempre que esse evento acontece.

Vários [Ouvintes de eventos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/) já vêm predefinidos como parte do SDK, mas também pode usar o `onChange()` método em qualquer component para obter o mesmo resultado. Isto também funciona com qualquer [Component personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) que tenha definido, sem necessidade de trabalho extra.

Por exemplo, a função seguinte verifica o `AvatarEquippedData` component na entity do player e executa uma função se o player alterar qualquer um dos seus wearables ou emotes equipados. Os novos valores do component são passados como argumentos da função.

```ts
import { AvatarEquippedData } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('Nova lista de wearables: ', equipped.wearableUrns)
		console.log('Nova lista de emotes: ', equipped.emoteUrns)
	})
}
```

Graças ao `onChange()` método, não é necessário criar um System e verificar iterativamente novos valores em cada frame; isto simplifica bastante este caso de utilização muito comum.


> [!WARNING]
> **📔 Nota**: Não use `onChange()` dentro de um System, pois isso subscreveria uma nova cópia da função em cada frame do game loop e poderia potencialmente levar a crashes.


O mesmo método funciona de imediato com [Component personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/). Por exemplo:

```ts
// definir component
export const MyComponent = engine.defineComponent('myComponent', {
	value1: Schemas.Boolean,
	value2: Schemas.Float,
})

// Uso
export function main() {
	// Criar entities
	const myEntity = engine.addEntity()

	// Criar instâncias do component
	MyComponent.create(myEntity, {
		value1: true,
		value2: 10,
	})

	// Subscrever às alterações
	MyComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.value1)
		console.log(componentData.value2)
	})
}
```

Também pode combinar esta abordagem com [Consultando components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/), para subscrever em lote cada entity na scene que tenha um determinado component à sua própria função.

```ts
export function main() {
	for (const [entity] of engine.getEntitiesWith(MyComponent)) {
		MyComponent.onChange(entity, (componentData) => {
			if (!componentData) return
			console.log(componentData.value1)
			console.log(componentData.value2)
		})
	}
}
```

Note que esta abordagem irá subscrever apenas `onChange()` para entities que existam no início da scene, por exemplo entities criadas através da UI do [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/).


> [!NOTE]
> **💡 Dica**: Se preferir, em vez disso, lidar com events que não estejam necessariamente relacionados com a alteração de um component, recomendamos importar a biblioteca TypeScript [Mitt](https://www.npmjs.com/package/mitt) para a sua scene. Esta biblioteca oferece funções simples para emitir e ouvir events.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/subscribe-to-changes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
