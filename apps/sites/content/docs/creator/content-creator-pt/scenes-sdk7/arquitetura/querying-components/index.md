> Saiba como obter listas de entities que têm components em comum, para facilitar a verificação ou atualização delas.

# Consultando Components

Você pode [consultar components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/) com o método `engine.getEntitiesWith(...components)` para acompanhar todas as entities na scene que têm certos components.

[Systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) normalmente iteram pelas entities nessas consultas, realizando as mesmas operações em cada uma. Ter um grupo predefinido de entities válidas é uma ótima forma de poupar recursos, especialmente para functions que são executadas a cada tick do game loop. Se, a cada tick, o seu system tivesse de iterar por cada entity na scene à procura das que precisa, isso seria muito ineficiente.

Você pode acessar as entities em uma query da seguinte forma.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	//...
}
```

## Componentes necessários

Ao fazer uma query, especifique quais components precisam estar presentes em cada entity adicionada ao group. Você pode listar quantos components quiser; a query só retornará entities que tenham **todos** dos components listados.

```ts
for (const [entity] of engine.getEntitiesWith(
	Transform,
	Physics,
	NextPosition
)) {
	//...
}
```


> [!NOTE]
> **💡 Dica**: Se a sua query retornar entities com as quais você não precisa lidar, considere criar um componente customizado para agir como um [flag](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/#components-as-flags). Esse componente não precisa ter nenhuma propriedade, mas pode ser usado para marcar um subgrupo específico de entities que você talvez queira tratar de forma diferente.


## Use queries em um system

```ts
// Defina um System
function PhysicsSystem(dt: number) {
	// query por entities que incluem tanto um componente Transform quanto um Physics
	for (const [entity] of engine.getEntitiesWith(Transform, Physics)) {
		const transform = Transform.getMutable(entity)
		const vel = Physics.get(entity).velocity
		transform.position.x += vel.x
		transform.position.y += vel.y
		transform.position.z += vel.z
	}
}

// Adicione o system ao engine
engine.addSystem(PhysicsSystem)
```

No exemplo acima, a `PhysicsSystem` function itera pelas entities na query, que é executada a cada tick do game loop.

* Se a scene tiver várias *ball* entities, cada uma com um `Transform` e uma `Physics` component, então elas serão tratadas, e sua posição será atualizada a cada tick.
* Se a sua scene também tiver outras entities, por exemplo um *hoop* e uma *scoreBoard* que tenham apenas um `Transform` mas não um `Physics` component, então elas não serão afetadas por `PhysicsSystem`.

## Lidando com as entities e components

A `getEntitiesWith` function retorna uma collection, que inclui referências a um conjunto de entities e também pode incluir, opcionalmente, referências aos components listados.

Usando a sintaxe mais simples, você obtém apenas uma lista de referências às entities correspondentes.

```ts
for (const [entity] of engine.getEntitiesWith(myComponent, myOtherComponent)) {
	//...
}
```

Ao iterar nessa lista de entities, você pode então obter versões somente leitura ou mutáveis dos seus components, usando `.get` ou `getMutable`.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	// obter versão somente leitura
	const transformReadOnly = Transform.get(entity)

	// obter versão mutável
	const transformMutable = Transform.getMutable(entity)
}
```

Opcionalmente, você também pode obter referências a cada um dos components listados diretamente como parte da collection retornada pela query. Para fazer isso, basta declarar várias referências juntas, uma para cada component que você quer obter. Adicionar essas referências é opcional, e você não precisa declarar referências aos *todos* components na query também.

```ts
// retorna referências à entity e ao primeiro component listado
for (const [entity, component1] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// iterar pela lista de entities
}

// retorna referências à entity e aos dois primeiros components listados
for (const [entity, component1, component2] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// iterar pela lista de entities
}
```


> [!WARNING]
> **📔 Nota**: Essas referências são somente leitura. Para obter versões mutáveis desses components, você precisa usar a function `.getMutable` fazendo referência à entity.


Você pode então se referir a essas referências conforme itera pela collection de resultados; em cada entrada, você terá acesso à entity e às suas referências de component correspondentes.

```ts
for (const [entity, transformReadOnly] of engine.getEntitiesWith(Transform)) {
	console.log('entity id: ', entity)
	console.log('has position : ', transformReadOnly.position)
}
```

## Subscribe to changes

Um caso de uso comum é executar uma function apenas caso os dados em um certo component mudem. Use a [OnChange](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/subscribe-to-changes/) function para evitar ter de definir um system e ter de comparar explicitamente valores antigos com novos values.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
