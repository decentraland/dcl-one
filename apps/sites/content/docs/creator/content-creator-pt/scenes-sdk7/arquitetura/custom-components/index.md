> Crie um component personalizado para lidar com dados específicos relacionados a uma entity

# Components Personalizados

Os dados de uma entity são armazenados na sua [components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/). O Decentraland SDK fornece uma série de components base que gerenciam diferentes aspectos de uma entity, como sua posição, forma, material etc. O engine sabe como interpretar as informações neles e mudará a forma como a entity é renderizada de acordo assim que os valores deles mudarem.

Se a lógica da sua scene exigir armazenar informações sobre uma entity que não seja tratada pelos components padrão do SDK, então você pode criar um tipo customizado de component na sua scene. Então você pode construir [systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) que verifiquem mudanças nesses components e respondam de acordo.

## Sobre definir components

Para definir um novo component, use `engine.defineComponent`. Cada component precisa do seguinte:

* Uma **componentName**: Um identificador de string único que o SDK usa internamente para identificar esse tipo de component. Pode ser qualquer string, desde que seja única.
* A **schema**: Uma classe que define a estrutura de dados mantida pelo component.
* **valores padrão** *(opcional)*: Um objeto contendo valores padrão a serem usados para inicializar uma cópia do component, quando eles não forem fornecidos.

```ts
export const WheelSpinComponent = engine.defineComponent('wheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```


> [!WARNING]
> **📔 Nota**: Custom Components devem sempre ser escritos fora do `main()` function, em um arquivo separado. Elas precisam ser interpretadas antes de `main()` é executado. O local recomendado para isso é em uma `/components` pasta dentro de `/src`, cada um em seu próprio arquivo. Dessa forma, fica mais fácil reutilizá-los em projetos futuros.


Depois de definir um custom component, você pode criar instâncias desse component, que fazem referência a entities na scene. Quando você cria uma instância de um component, você fornece valores para cada um dos campos no schema do component. Os valores devem estar de acordo com os tipos declarados de cada campo.

```ts
// Criar entities
const wheel1 = engine.addEntity()
const wheel2 = engine.addEntity()

// Criar instâncias do component
WheelSpinComponent.create(wheel1, {
	spinning: true,
	speed: 10,
})

WheelSpinComponent.create(wheel2, {
	spinning: false,
	speed: 0,
})
```

Cada entity à qual o component é adicionado instancia uma nova cópia do component, contendo dados específicos para essa entity.

Seu custom component também pode executar as outras funções comuns disponíveis em outros components:

```ts
// Obter uma instância somente leitura do component a partir de uma entity
const readOnlyInstance = MyCustomComponent.get(myEntity)

// Obter uma instância mutável do component a partir de uma entity
const mutableInstance = MyCustomComponent.getMutable(myEntity)

// Excluir a instância do component de uma entity
MyCustomComponent.deleteFrom(myEntity)
```

## Sobre o componentName

Cada component deve ter um nome ou identificador único, que o diferencie internamente. Você não precisará usar esse identificador interno em nenhum outro lugar do seu code. Uma boa prática é usar o mesmo nome que você atribui ao component, mas começando com uma letra minúscula; porém, o que realmente importa é que esse identificador seja único dentro do projeto.

Ao criar components que serão compartilhados como parte de uma library, tenha em mente que os nomes dos components na sua library não devem se sobrepor a nenhum nome de component no projeto onde ela está sendo usada, nem em outras libraries que também são usadas por esse projeto. Para evitar o risco de qualquer sobreposição, a melhor prática recomendada é incluir o nome da library como parte da `componentName` string. Você pode seguir esta fórmula: `${packageName}::${componentName}`. Por exemplo, se você criar uma`MyUtilities` library que inclui um `MoveEntity` component, defina o `componentName` desse component para `MyUtilities::moveEntity`.

## Components como flags

Você pode querer adicionar um component que apenas marque uma entity para diferenciá-la das outras, sem usá-lo para armazenar dados. Para isso, deixe o schema como um objeto vazio.

Isso é especialmente útil ao usar [consultas de components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/). Um component de flag simples pode ser usado para distinguir entities das outras e evitar que o system itere sobre mais entities do que o necessário.

```ts
export const IsEnemyFlag = engine.defineComponent('isEnemyFlag', {})
```

Então você pode criar um system que itera sobre todas as entities com esse component.

```ts
export function handleEnemies() {
	for (const [entity] of engine.getEntitiesWith(IsEnemyFlag)) {
		// fazer algo em cada entity
	}
}

engine.addSystem(handleEnemies)
```

## Schemas de components

Um schema descreve a estrutura dos dados dentro de um component. Um component pode armazenar quantos campos você quiser, e cada um deve estar incluído na estrutura do schema. O schema pode incluir quantos níveis de itens aninhados forem necessários.

Cada campo no schema deve incluir uma declaração de tipo. Você só pode usar os tipos especiais de schema fornecidos pelo SDK. Por exemplo, use o tipo `Schemas.Boolean` em vez do tipo `boolean`. Escreva `Schemas.` e sua IDE exibirá todas as opções disponíveis.

```ts
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```

O exemplo acima define um component cujo schema contém dois valores, um `spinning` booleano e um `speed` número de ponto flutuante.

Você pode optar por criar o schema inline ao definir o component, ou, para maior legibilidade, pode criá-lo e depois referenciá-lo.

```ts
// Opção 1: definição inline
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// Opção 2: definir schema e component separadamente

//// schema
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// component
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema
)
```


> [!NOTE]
> **💡 Dica**: Ao criar uma instância de um component, as opções de autocompletar do VS Studio sugerirão quais campos você pode adicionar ao component pressionando *Ctrl + Space*.


### Tipos de schema padrão

Os seguintes tipos básicos estão disponíveis para uso dentro dos campos de um schema:

* `Schemas.Boolean`
* `Schemas.Byte`
* `Schemas.Double`
* `Schemas.Float`
* `Schemas.Int`
* `Schemas.Int64`
* `Schemas.Number`
* `Schemas.Short`
* `Schemas.String`
* `Schemas.Entity`

Os seguintes tipos complexos também existem. Cada um inclui uma série de propriedades aninhadas com valores numéricos.

* `Schemas.Vector3`
* `Schemas.Quaternion`
* `Schemas.Color3`
* `Schemas.Color4`


> [!NOTE]
> **💡 Dica**: Veja [Tipos de geometria](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/special-types/) e [Tipos de cor](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/) para mais detalhes sobre como esses tipos de dados são úteis.


Por exemplo, você pode usar esses tipos de schema em um component como este para acompanhar o movimento gradual de uma entity. Este component armazena uma posição inicial e uma posição final como valores Vector3, além de uma velocidade e a fração do percurso concluído como números de ponto flutuante. Veja [Mover entities](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/#move-between-two-points) a implementação completa deste exemplo.

```ts
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)
```

### Tipos de array

Para definir o tipo de um campo como um array, use `Schemas.Array()`. Passe o tipo dos elementos no array como uma propriedade.

```ts
const MySchema = {
	numberList: Schemas.Array(Schemas.Int),
}
```

Quando você lê de volta um campo de array com `MyComponent.get()`, o array é somente leitura. Métodos que o alteram no próprio lugar, como `.push()`, não estão disponíveis. Use `MyComponent.getMutable()` quando precisar alterar o conteúdo.

### Campos opcionais

Usa `Schemas.Optional()` para permitir que um campo contenha um valor ou `undefined`.

```ts
const MySchema = {
	playerId: Schemas.Optional(Schemas.String),
	score: Schemas.Optional(Schemas.Int),
}
```

Apenas `undefined` conta como "não definido". Valores falsy como `false`, `0`, e `''` são armazenados e lidos exatamente como foram escritos.

### Tipos de schema aninhados

Para definir o tipo de um campo como um objeto, use `Schemas.Map()`. Passe o conteúdo desse objeto como uma propriedade. Esse objeto aninhado é essencialmente um schema por si só, aninhado dentro do schema pai.

```ts
const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: Schemas.Map({
		nestedField1: Schemas.Boolean,
		nestedField2: Schemas.Boolean,
	}),
}
```

Alternativamente, para manter as coisas mais legíveis e reutilizáveis, você poderia obter o mesmo resultado definindo o schema aninhado separadamente e depois referenciando-o ao definir o schema pai.

```ts
const MyNestedSchema = Schemas.Map({
	nestedField1: Schemas.Boolean,
	nestedField2: Schemas.Boolean,
})

const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: MyNestedSchema,
}
```

### Tipos de enums

Você pode definir o tipo de um campo em um schema como um enum. Os enums facilitam a seleção entre um número finito de opções, fornecendo valores legíveis por pessoas para cada um.

Para definir o tipo de um campo como um enum, primeiro você deve definir o enum. Depois, você pode referenciá-lo usando `Schemas.EnumNumber` ou `Schemas.EnumString`, dependendo do tipo de enum. Essas funções recebem dois parâmetros: o enum a ser referenciado e um valor padrão a ser usado para esse campo.

```ts
//// enum de String

// Definir enum
enum Color {
	Red = 'red',
	Green = 'green',
	Pink = 'pink',
}

// Definir um component que usa esse enum em um campo
const ColorComponent = engine.defineComponent('Color', {
	color: Schemas.EnumString<Color>(Color, Color.Red),
})

// Usar o component em uma entity
ColorComponent.create(engine.addEntity(), { color: Color.Green })

//// enum de Number

// Definir enum
enum CurveType {
	LINEAR,
	EASEIN,
	EASEOUT,
}

// Definir um component que usa esse enum em um campo
const CurveComponent = engine.defineComponent('curveComponent', {
	curve: Schemas.EnumNumber<CurveType>(CurveType, CurveType.LINEAR),
})

// Usar o component em uma entity
CurveComponent.create(engine.addEntity(), { curve: CurveType.EASEIN })
```

### Tipos intercambiáveis

Você pode definir o tipo de um campo em um schema para seguir um `oneOf` padrão, no qual diferentes tipos podem ser aceitos.

```ts
const MySchema = {
	myField: Schemas.OneOf({ type1: Schemas.Vector3, type2: Schemas.Quaternion }),
}

export const MyComponent = engine.defineComponent('MyComponent', MySchema)
```

Ao criar uma instância do component, você precisa especificar o tipo selecionado com um `$case`, por exemplo:

```ts
MyComponent.create(myEntity, {
	myField: {
		$case: 'type1',
		value: Vector3.create(1, 1, 1),
	},
})
```

Deixar o campo sem definir também é válido. Um `OneOf` field sem `$case` e é lido de volta como um objeto vazio, `{}`.

### Components de um único tipo

Um component não precisa conter um objeto com vários campos. Para definir um que contenha um único valor, use `engine.defineComponentFromSchema()` e passe o tipo diretamente:

```ts
// Um component que armazena um número por entity
export const Score = engine.defineComponentFromSchema('my-scene::Score', Schemas.Int)

// Um component que armazena uma lista de números por entity
export const History = engine.defineComponentFromSchema(
	'my-scene::History',
	Schemas.Array(Schemas.Int)
)
```

Eles se comportam como qualquer outro component, inclusive quando o valor armazenado é falsy. Um `Score` de `0` é um component que existe e contém `0`, e não um component ausente.

## Valores padrão

Geralmente é bom ter valores padrão em seus components, para que não seja necessário definir explicitamente cada valor toda vez que você criar uma nova cópia.

A `engine.defineComponent()` function recebe um terceiro argumento, que permite passar um objeto com valores a serem usados por padrão. Esse objeto pode incluir alguns ou todos os valores do schema. Os campos não cobertos pelos padrões ou pelos valores que você fornecer ao inicializar uma cópia do component são inicializados com um valor parecido com zero, como `0`, `false`, ou com uma string vazia, dependendo do tipo.

```ts
// Definição

//// schema
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// padrões
const myDefaultValues = {
	spinning: true,
	speed: 1,
}

//// component
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema,
	myDefaultValues
)

// Uso
export function main() {
	//// Criar entities
	const wheel = engine.addEntity()
	const wheel2 = engine.addEntity()

	//// inicializar component usando valores padrão
	WheelSpinComponent.create(wheel)

	//// inicializar component com um valor customizado, usando o padrão para os demais
	WheelSpinComponent.create(wheel2, { speed: 5 })
}
```

O exemplo acima cria um `WheelSpinComponent` component que inclui tanto um schema quanto um conjunto de valores padrão a serem usados. Se você então inicializar uma cópia desse component sem especificar nenhum valor, ela usará os definidos no padrão.

## Inscreva-se nas alterações

Um caso de uso comum é executar uma função apenas quando os dados em um determinado componente mudarem. Use a [OnChange](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/subscribe-to-changes/) função para evitar ter de definir um system e comparar explicitamente os valores antigos com os novos.

```ts
export function main() {
	//Criar entity, etc

	WheelSpinComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.speed)
		console.log(componentData.spinning)
	})
}
```

## Construindo systems para usar um component

Com seu component definido e adicionado a entities na sua scene, você pode criar [Systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) para executar lógica, fazendo uso desses dados armazenados no component.

```ts
// definir component
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// Uso
export function main() {
	// Criar entities
	const wheel1 = engine.addEntity()
	const wheel2 = engine.addEntity()

	// Criar instâncias do component
	WheelSpinComponent.create(wheel1, {
		spinning: true,
		speed: 10,
	})

	WheelSpinComponent.create(wheel2, {
		spinning: false,
		speed: 0,
	})
}

// Definir um system para iterar sobre essas entities
export function spinSystem(dt: number) {
	// iterar sobre todas as entities com um WheelSpinComponent
	for (const [entity, wheelSpin] of engine.getEntitiesWith(
		WheelSpinComponent
	)) {
		// fazer algo somente se spinning == true
		if (wheelSpin.spinning) {
			// obter um componente Transform mutável
			const transform = Transform.getMutable(entity)

			// atualizar o valor da rotação de acordo
			transform.rotation = Quaternion.multiply(
				transform.rotation,
				Quaternion.fromAngleAxis(dt * wheelSpin.speed, Vector3.Up())
			)
		}
	}
}

// Adicione o System ao engine
engine.addSystem(spinSystem)
```

O exemplo acima define um system que itera sobre todas as entities que incluem o custom `wheelSpinComponent`, e as gira ligeiramente a cada tick do game loop. A quantidade dessa rotação é proporcional ao `speed` valor armazenado na instância do component em cada entity. O exemplo faz uso de [consultas de components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/) para obter apenas as entities relevantes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
