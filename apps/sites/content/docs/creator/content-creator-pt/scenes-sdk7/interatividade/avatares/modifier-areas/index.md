> Oculte avatares, oculte nametags ou desative passaportes para jogadores dentro de uma região da sua scene

# Áreas de Modificador de Avatar

Os avatares se comportam e têm uma aparência consistente em Decentraland à medida que atravessam as cenas. No entanto, você pode adicionar um `AvatarModifierArea` a uma região da sua cena para afetar o comportamento dos avatares dos jogadores quando entrarem nessa área.


> [!CAUTION]
> **❗Aviso**\
> Limite a quantidade de `AvatarModifierAreas` que você usa na sua cena a apenas algumas. Se você usar muitas delas, isso pode ter um impacto significativo no desempenho.


### Posicionando Avatar Modifier Areas

Adicione uma Entity com um `AvatarModifierArea` component e posicione essa Entity usando um `Transform` componente.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Ao criar um `AvatarModifierArea` component, você deve fornecer o seguinte:

* `área`: Tamanho da área do modificador
* `modifiers`: Um array que lista os modificadores a implementar na área. Esta propriedade usa valores do `AvatarModifierType` enum.

Os modificadores suportados são:

* `AvatarModifierType.AMT_HIDE_AVATARS`
* `AvatarModifierType.AMT_DISABLE_PASSPORTS`
* `AvatarModifierType.AMT_HIDE_NAMETAGS`

Todos os efeitos de um `AvatarModifierArea` só acontecem dentro da região da sua área. Os jogadores voltam ao normal quando saem da área.

Uma chamada de `AvatarModifierArea` afeta apenas os jogadores que estão dentro da área. Entrar na área não afeta a forma como os outros jogadores fora da área são percebidos.

Os efeitos de um `AvatarModifierArea` são calculados localmente para cada jogador. Você pode ter um `AvatarModifierArea` que está presente na cena apenas para alguns jogadores e não para outros. Por exemplo, você poderia fazer um jogo de "marco polo", em que apenas um jogador na cena tenha uma área de modificador que oculte todos os outros jogadores. Todos os outros jogadores que não tenham essa área de modificador em sua versão local da cena conseguem se ver normalmente.

Se a área ocultar avatares, os jogadores que não tiverem a área em sua versão local da cena verão todos os avatares normalmente, até mesmo aqueles que se veem como ocultos. Os jogadores que tiverem a área se verão e verão todos os outros avatares como afetados pela área quando entrarem nela.


> [!WARNING]
> **📔 Nota**: As Avatar modifier areas são afetadas pelo *posição* e *rotação* do component Transform da Entity hospedeira, mas não são afetadas pelo *escala*.



> [!WARNING]
> **📔 Nota**: As unidades `AvatarModifierArea`componente deve ser importado via
> 
> > `import { AvatarModifierArea } from "@dcl/sdk/ecs"`
> 
> Consulte [Importações](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


### Ocultar avatares

Quando um jogador entra em um `AvatarModifierArea` que tem o `AvatarModifierType.AMT_HIDE_AVATARS` modificador, o avatar do jogador deixa de ser renderizado. Isso se aplica tanto ao jogador em visão em 3ª pessoa quanto quando outros jogadores entram na área.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Isso permite substituir o avatar padrão do Decentraland por qualquer avatar personalizado que você queira mostrar na sua cena; veja [Substituir o avatar do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars/#replace-the-players-avatar). Observe que, se quiser ver outros jogadores com avatares personalizados, você deve cuidar da sincronização das posições dos jogadores por conta própria.

### Desativar o pop-up do Passport

Quando um jogador entra em um `AvatarModifierArea` que tem o `AvatarModifierType.AMT_DISABLE_PASSPORTS` modificador, clicar neles não abre mais a UI do passport que mostra a bio do jogador, o inventário etc.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Isso é especialmente útil em jogos em que abrir essa UI acidentalmente poderia interromper o ritmo do jogo, por exemplo em um jogo de tiro multiplayer.

### Ocultar nametags

Quando um jogador entra em um `AvatarModifierArea` que tem o `AvatarModifierType.AMT_HIDE_NAMETAGS` modificador, o nametag do jogador é ocultado enquanto o próprio avatar permanece visível.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Isso é útil para palcos, apresentações ou cenas roteirizadas em que você quer ocultar os nametags dos jogadores sem esconder os avatares em si. Por exemplo, você pode querer uma experiência visual limpa durante uma apresentação, em que os avatares ficam visíveis, mas os nomes flutuantes não distraiam os espectadores.

`AMT_HIDE_NAMETAGS` também oculta quaisquer placas personalizadas adicionadas com o [`AvatarNametag`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-nametags/) componente.


> [!NOTE]
> **💡 Dica**: `AMT_HIDE_AVATARS` já oculta os nametags junto com o avatar, então você não precisa adicionar `AMT_HIDE_NAMETAGS` ao usar `AMT_HIDE_AVATARS`. Use `AMT_HIDE_NAMETAGS` somente quando quiser ocultar os nametags mantendo os avatares visíveis.


Você também pode combinar `AMT_HIDE_NAMETAGS` com outros modificadores como `AMT_DISABLE_PASSPORTS` na mesma área:

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!NOTE]
> **💡 Dica**: O nametag só fica oculto enquanto a cabeça ou o torso do jogador estiver dentro da área. Se a área for muito baixa e o jogador der um pulo duplo por cima dela, o nametag reaparecerá brevemente. Torne a área alta o suficiente para cobrir a faixa de movimento esperada.


### Excluir avatares

Você pode excluir uma lista de jogadores de ser afetada por uma modifier area adicionando os IDs dos jogadores a um array na `excludeIds` propriedade da área de modificador.

Este exemplo oculta todos os avatares em uma área, exceto os dos jogadores com IDs específicos. Você poderia usar isso, por exemplo, em um evento ao vivo, para mostrar apenas os anfitriões do evento no palco e ocultar quaisquer outros jogadores que pulem para o palco.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: ['0xx1...', '0xx2...'],
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!WARNING]
> **📔 Nota**: Certifique-se de que os IDs dos jogadores estejam todos escritos com letras minúsculas. Use `.toLowerCase()` se necessário.


As modifier areas executam localmente na instância de cada jogador. A lista de IDs excluídos pode ser diferente para cada jogador. No exemplo abaixo, cada jogador exclui o próprio ID de um modificador que oculta avatares, para que cada um veja o próprio avatar e nenhum outro.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	let userData = getPlayer()
	if (!userData) return

	const entity = engine.addEntity()

	AvatarModifierArea.create(entity, {
		area: Vector3.create(16, 5, 16),
		modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
		excludeIds: [userData.userId],
	})

	Transform.create(entity, {
		position: Vector3.create(8, 0, 8),
	})
}
```


> [!CAUTION]
> **❗Aviso**\
> Se a lista de IDs excluídos for alterada periodicamente (por exemplo, com base em jogadores entrando ou saindo de uma área), certifique-se de que a lista seja mantida em ordem. Faça um `.sort()` no array, para que a lista permaneça na mesma ordem sempre que for passada. Dessa forma, apenas as mudanças na lista são calculadas. Caso contrário, isso pode ter um impacto significativo no desempenho da cena.
> 
> ```ts
> AvatarModifierArea.create(entity, {
> 	area: Vector3.create(16, 5, 16),
> 	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
> 	excludeIds: myAvatarList.sort(),
> })
> ```
> 


### Áreas sobrepostas e alterações em tempo de execução

Cada avatar carrega um sinalizador liga/desliga por efeito, não uma contagem das áreas em que está. Cada área grava esse mesmo sinalizador, e a última gravação vence. Duas consequências costumam pegar as pessoas de surpresa:

* **Sair de uma área faz o avatar reaparecer, mesmo enquanto ele ainda estiver dentro de outra.** Sair de uma área pequena que oculta avatares faz o avatar ficar visível novamente, embora uma área de ocultação em toda a cena ainda cubra aquele ponto.
* **Uma área cujo `excludeIds` lista um jogador escreve "visible" para ele ao entrar**, para cada efeito que a própria área listar. Isso substitui uma área que oculta todos em que ele também esteja.

Prefira uma área de cada vez por avatar. Se as áreas se sobrepuserem, faça com que elas concordem em cada efeito.

**Remover um modificador no local não o desfaz.** Quando você altera os campos de uma área, o client reaplica os modificadores que estão listados agora. Nada restaura um efeito cujo modificador você removeu da lista, então esvaziar `modifiers`, ou trocar `AMT_HIDE_AVATARS` por `AMT_HIDE_NAMETAGS`, deixa os avatares ocultos. Adicionar um modificador no local funciona.

Para trocar uma área por outro conjunto de modificadores, exclua a Entity e crie a substituta no mesmo frame:

```ts
import { AvatarModifierArea, AvatarModifierType, engine, Entity, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Substitua a área em vez de editar seus modificadores no local
function setModifiers(oldArea: Entity, modifiers: AvatarModifierType[]): Entity {
	engine.removeEntity(oldArea) // isso também faz reaparecer todos os avatares dentro dela

	const area = engine.addEntity()
	Transform.create(area, { position: Vector3.create(8, 1, 8) })
	AvatarModifierArea.create(area, {
		area: Vector3.create(16, 5, 16),
		modifiers,
		excludeIds: [],
	})
	return area
}
```

### Depurar áreas de modificador

Pode ser difícil saber exatamente quais partes da cena suas modifier areas cobrem com base apenas no código. O feedback visual ajuda muito a confirmar que elas estão bem posicionadas.

Para verificar as posições de um `AvatarModifierArea` ou um `CameraModeArea`, dê à Entity que o contém um `MeshRenderer` component com uma `caixa` shape, e defina a scale para o mesmo tamanho da `área` da área de modificador.


> [!WARNING]
> **📔 Nota**: As modifier areas não são afetadas pela `escala` propriedade do transform; seu tamanho é baseado em sua `área` propriedade.


```ts
const entity = engine.addEntity()
const areaSize = Vector3.create(8, 3, 8)

AvatarModifierArea.create(entity, {
	area: areaSize,
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
  	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
	scale: areaSize,
})

MeshRenderer.setBox(entity)
Material.setPbrMaterial(entity, {
	albedoColor: Color4.create(0.5, 0.5, 0.5, 0.5),
})
```

Para ativar os efeitos da área de modificador, a cabeça ou o torso do jogador deve entrar na área. Isso não terá efeito se apenas os pés do jogador estiverem na área. Certifique-se de que o jogador não consiga evitar a área facilmente pulando.


> [!WARNING]
> **📔 Nota**: A área completa deve caber dentro dos limites da sua cena.



> [!NOTE]
> **💡 Dica**: Para ver exemplos funcionando, consulte a [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas) cena de teste, que combina `AvatarModifierArea` com runtime-mutated `excludeIds`e [`10,99-avatar-modifier-hide-nametags`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/10,99-avatar-modifier-hide-nametags), que isola o `AMT_HIDE_NAMETAGS` modificador.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
