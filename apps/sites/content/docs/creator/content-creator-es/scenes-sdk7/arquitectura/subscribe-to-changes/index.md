> Detecta cambios en un componente y ejecuta funciones en cada cambio

# Suscribirse a cambios

Una forma limpia de escribir tu código es suscribirte a eventos y ejecutar una función cada vez que ocurra ese evento.

Un número de [escuchadores de eventos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/) vienen predefinidos como parte del SDK, pero también puedes usar el `onChange()` método en cualquier component para lograr lo mismo. Esto también funciona con cualquier [Componente personalizado](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) que hayas definido, sin necesidad de trabajo extra.

Por ejemplo, la siguiente función comprueba el `AvatarEquippedData` componente de la entidad del jugador, y ejecuta una función si el jugador cambia alguno de sus wearables o emotes equipados. Los nuevos valores del componente se pasan como argumentos de la función.

```ts
import { AvatarEquippedData } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('Nueva lista de wearables: ', equipped.wearableUrns)
		console.log('Nueva lista de emotes: ', equipped.emoteUrns)
	})
}
```

Gracias al `onChange()` método, no es necesario crear un System e ir comprobando iterativamente nuevos valores en cada frame; simplifica mucho este caso de uso tan común.


> [!WARNING]
> **📔 Nota**: No uses `onChange()` dentro de un System, ya que eso suscribiría una nueva copia de la función en cada frame del game loop y podría provocar fallos.


El mismo método funciona de inmediato con [Componente personalizado](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/). Por ejemplo:

```ts
// definir Component
export const MyComponent = engine.defineComponent('myComponent', {
	value1: Schemas.Boolean,
	value2: Schemas.Float,
})

// Uso
export function main() {
	// Crear entidades
	const myEntity = engine.addEntity()

	// Crear instancias del Component
	MyComponent.create(myEntity, {
		value1: true,
		value2: 10,
	})

	// Suscribirse a los cambios
	MyComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.value1)
		console.log(componentData.value2)
	})
}
```

También puedes combinar este enfoque con [Consulta de components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/), para suscribirte en bloque a cada entidad de la escena que tenga un cierto component a su propia función.

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

Ten en cuenta que este enfoque solo se suscribirá a `onChange()` las entidades que existen al inicio de la escena, por ejemplo, entidades creadas mediante la UI del [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/).


> [!NOTE]
> **💡 Consejo**: Si prefieres manejar en su lugar eventos que no estén necesariamente relacionados con un cambio en un component, recomendamos importar la librería TypeScript [Mitt](https://www.npmjs.com/package/mitt) en tu escena. Esta librería ofrece funciones simples para emitir y escuchar eventos.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/subscribe-to-changes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
