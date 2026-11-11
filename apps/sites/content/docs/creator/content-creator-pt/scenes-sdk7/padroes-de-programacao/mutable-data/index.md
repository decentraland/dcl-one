> Saiba como lidar com dados somente leitura e mutáveis de components

# Dados Mutáveis

Ao referenciar dados de um [componente](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/), você pode obter tanto a versão mutável quanto a versão somente leitura (imutável).

Você deve sempre lidar com as versões somente leitura dos dados sempre que possível. Essa prática pode trazer uma melhoria muito significativa no desempenho da sua scene, quando comparada a lidar sempre com versões mutáveis desses mesmos dados.

A `.get()` função em um componente retorna uma versão somente leitura (imutável) do componente. Você só pode ler seus valores, mas não pode alterar nenhuma de suas propriedades.

A `.getMutable()` função retorna uma versão do componente que permite alterar seus valores. Use versões mutáveis somente quando você pretender fazer alterações em um componente; caso contrário, use sempre `get()`.

```ts
// buscar uma versão somente leitura (imutável)
const immutableTransform = Transform.get(myEntity)

// o seguinte NÃO funciona:
// immutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// a linha seguinte DE FATO altera a posição da entidade
mutableTransform.position.y = 2
```

Uma boa prática é iterar sobre componentes somente leitura para verificar valores e, em seguida, buscar a versão mutável de um componente individual apenas quando for necessária uma alteração.

```ts
// altura máxima codificada
const MAX_HEIGHT = 10

// Defina o sistema
function HeightLimitSystem(dt: number) {
	// iterar sobre todas as entities que têm um componente Transform
	for (const [entity] of engine.getEntitiesWith(Transform)) {
		// obter valores somente leitura
		const currentHeight = Transform.get(entity).position.y

		// comparar valores
		if (currentHeight > MAX_HEIGHT) {
			// buscar a versão mutável para fazer uma alteração
			const mutableTransform = Transform.getMutable(entity)

			// alterar Transform
			mutableTransform.position.y = MAX_HEIGHT
		}
	}
}

// Adicione o sistema ao engine
engine.addSystem(HeightLimitSystem)
```

No exemplo acima, um system verifica os valores somente leitura de um componente de uma entidade `Transform` componente. A cada tick, ele verifica se a posição *y* é maior que uma altura máxima codificada. Se a altura no Transform estiver acima desse limite, então, e somente então, buscamos a versão mutável do Transform. Isso pode parecer trabalho extra para a scene, mas em uma scene em que verificamos valores a cada tick do loop do jogo e só fazemos alterações ocasionalmente, isso resulta em enormes ganhos de desempenho.

Essa prática segue os princípios de [programação orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/). Também está sendo gradualmente adotada como uma prática padrão na indústria de jogos, por causa da grande melhoria que ela proporciona.


> [!WARNING]
> **📔 Nota**: Em versões mais antigas do SDK (6.x ou anteriores), components eram sempre tratados como mutáveis. Esse padrão pode ser um pouco mais direto de aprender, mas era muito menos eficiente de executar.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/mutable-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
