> Mostre um rótulo personalizado acima do nametag de um avatar, para exibir uma classificação, uma função ou uma equipe.

# Nametags do Avatar

O `AvatarNametag` o componente adiciona uma pequena placa acima da etiqueta de nome regular de um avatar, mostrando o texto que sua cena escolher. Use-o para mostrar um rank, uma função, uma equipe ou qualquer outra coisa que sua cena acompanhe sobre um jogador, como "VIP", "Equipe Vermelha" ou "Dono do Clube".

<figure><img src="/docs/_remote/bb915fa6311eac2059c6e8e8.webp" alt="An avatar with a Club Owner plate floating above its regular nametag"><figcaption><p>Uma placa fornecida pela cena acima da etiqueta de nome regular do jogador</p></figcaption></figure>

Esta página explica como adicionar uma placa, estilizar e mantê-la sincronizada conforme os jogadores entram e saem.


> [!WARNING]
> **📔 Nota**: `AvatarNametag` requer `@dcl/sdk` a versão 7.28.0 ou mais recente.


## Adicione uma placa ao jogador

O único campo obrigatório é `label`, o texto a mostrar.

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'

// Mostra "Dono do Clube" acima da etiqueta de nome do jogador atual
AvatarNametag.create(engine.PlayerEntity, {
	label: 'Dono do Clube',
})
```

A placa usa por padrão as cores da etiqueta de nome do próprio cliente, então ela combina com o restante da interface sem nenhum trabalho extra.


> [!NOTE]
> **💡 Dica**: Você não precisa esperar o avatar terminar de carregar antes de escrever o componente. Se o avatar ainda estiver carregando, a placa é aplicada assim que ele estiver pronto.


## Onde o componente funciona

`AvatarNametag` só faz algo em uma entidade que tenha um avatar:

* `engine.PlayerEntity`, o jogador executando a cena.
* A entidade de outro jogador, encontrada por meio do seu [`PlayerIdentityData`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/) component.
* Um [avatar de NPC](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars/), ou seja, qualquer entidade com um `AvatarShape` component.

Em qualquer outra entidade, o componente é ignorado.


> [!WARNING]
> **📔 Nota**: A placa é local para o cliente que a desenha. Ela nunca é enviada para outros jogadores. Se você quiser que todos vejam as mesmas placas, cada cliente deve executar a mesma lógica, com base em dados que todos possam ler. Veja [Placas em cenas multijogador](#plates-in-multiplayer-scenes).


## Altere as cores

Três campos opcionais recebem um [Color3](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/):

* `labelColor`: a cor do texto. O padrão é a cor nativa do texto da etiqueta de nome do cliente.
* `backgroundColor`: o preenchimento da placa. O padrão é a cor nativa de fundo da etiqueta de nome do cliente.
* `borderColor`: o contorno da placa. O padrão é `backgroundColor`, então não há borda visível a menos que você a defina.

`Color3` não tem canal alfa, e o componente não tem campo de opacidade, então a placa é sempre totalmente opaca.

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'
import { Color3 } from '@dcl/sdk/math'

// Um rótulo branco em uma placa azul-escura, com uma borda azul-clara
AvatarNametag.create(engine.PlayerEntity, {
	label: 'Estudante',
	labelColor: Color3.White(),
	backgroundColor: Color3.create(0.1, 0.2, 0.6),
	borderColor: Color3.create(0.78, 0.85, 1),
})
```

Para voltar às cores nativas de um campo, escreva o componente novamente sem esse campo. Use `createOrReplace` em vez de `getMutable`, porque substituir o componente inteiro é o que limpa os campos que você deixou de fora:

```ts
// Mantém o rótulo, remove as cores personalizadas e retorna ao visual nativo
AvatarNametag.createOrReplace(engine.PlayerEntity, { label: 'Estudante' })
```

## Remova uma placa

Use `deleteFrom` para remover a placa. A etiqueta de nome regular do avatar não é afetada:

```ts
AvatarNametag.deleteFrom(engine.PlayerEntity)
```

## Comportamento do rótulo

Alguns detalhes valem a pena conhecer antes de escolher o texto do rótulo:

* **Os rótulos têm uma única linha.** Eles nunca quebram linha.
* **Rótulos longos são truncados** com reticências. Mantenha os rótulos com uma ou duas palavras.
* **Um rótulo vazio** (`''`) desenha a placa sem texto nela.
* **Os espaços são mantidos**, então um rótulo composto apenas por espaços alarga uma placa sem texto. Esta é uma forma de dimensionar uma placa de cor sólida.
* **Para ocultar a palavra, mas manter sua largura**, defina `labelColor` com o mesmo valor que `backgroundColor`.


> [!NOTE]
> **💡 Dica**: Uma placa não precisa de nenhum texto. Para mostrar uma placa codificada por cor sozinha, por exemplo para marcar a filiação a uma equipe, defina `label` como uma string de espaços e escolha um `backgroundColor`. Mais espaços deixam a placa mais larga.
> 
> ```ts
> AvatarNametag.createOrReplace(engine.PlayerEntity, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


## Marque os jogadores à medida que chegam

A maneira mais simples de marcar outros jogadores é anexar o componente quando eles entrarem na cena, usando o `onEnterScene` evento. Ele fornece uma entidade nova a cada vez, então não há nada para procurar nem armazenar em cache:

```ts
import { onEnterScene } from '@dcl/sdk/players'
import { AvatarNametag } from '@dcl/sdk/ecs'

export function main() {
	onEnterScene((player) => {
		AvatarNametag.createOrReplace(player.entity, { label: 'VIP' })
	})
}
```

Veja [Obtenha dados do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/) para saber mais sobre acompanhar jogadores à medida que entram e saem.

## Marque todos os jogadores na cena

Para mostrar uma placa sobre todos de uma vez, percorra as entidades que têm um `PlayerIdentityData` component.

Reúna as entidades em um array **antes** de escrever nelas. Escrever um componente enquanto ainda estiver iterando em `engine.getEntitiesWith()` pode invalidar a consulta no meio do processo.

```ts
import { AvatarNametag, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

const LABELS = ['Equipe Azul', 'Equipe Vermelha']

function tagAllPlayers() {
	// 1. Reúna primeiro
	const players: { entity: Entity; address: string }[] = []
	for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
		players.push({ entity, address: identity.address.toLowerCase() })
	}

	// Classificar por endereço faz com que cada cliente chegue ao mesmo resultado,
	// independentemente da ordem em que os jogadores entraram
	players.sort((a, b) => a.address.localeCompare(b.address))

	// 2. Depois escreva, mas apenas quando o rótulo realmente mudar
	players.forEach(({ entity }, index) => {
		const label = LABELS[index % LABELS.length]
		if (AvatarNametag.getOrNull(entity)?.label === label) return
		AvatarNametag.createOrReplace(entity, { label })
	})
}
```

Comparar com o valor atual antes de escrever evita atualizações desnecessárias do componente a cada passagem.

### Acompanhe jogadores entrando e saindo

Os IDs das entidades dos jogadores são **não estáveis entre desconexões**. Quando um jogador sai, o ID da sua entidade é reciclado e pode ser atribuído ao próximo jogador que entrar, então uma placa escrita em uma entidade desatualizada acaba sobre o avatar errado.

Duas regras mantêm isso correto:

* Resolva a entidade novamente logo antes de **cada** escrita, em vez de armazenar a entidade em cache.
* Remova o componente quando um jogador sair.

Executar novamente a função de marcação em um timer captura jogadores que entram depois e nunca mantém entidades desatualizadas em cache. Uma vez por segundo é frequente o suficiente:

```ts
let timer = 0

engine.addSystem((dt: number) => {
	timer += dt
	if (timer < 1) return
	timer = 0
	tagAllPlayers()
})
```

## Placas em cenas multijogador

As placas são locais para o cliente de cada jogador. A placa nunca é enviada para outros jogadores; a cópia da cena de cada jogador calcula suas próprias placas. Se todos os jogadores devem ver as mesmas tags, a lógica que as atribui precisa produzir o mesmo resultado em cada cliente a partir de dados que todos compartilham. Classificar os jogadores por endereço da Wallet, como no exemplo acima, é uma maneira de fazer isso. Caso contrário, sua cena precisa sincronizar as atribuições por conta própria; veja [multijogador sem servidor](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/).

## Interação com etiquetas de nome ocultas

Um [`AvatarModifierArea`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/modifier-areas/#hide-nametags) com o `AMT_HIDE_NAMETAGS` o modificador oculta essas placas junto com as etiquetas de nome regulares. O mesmo se aplica a `AMT_HIDE_AVATARS`, que oculta o avatar inteiro.

Se você quiser que um avatar mostre **apenas** sua placa e nenhum nome abaixo dela, defina o `AvatarShape` campo do componente `name` como uma string vazia. Veja [Avatares NPC](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars/#add-a-label-above-the-name).

## Páginas relacionadas

* [Áreas de modificação do avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/modifier-areas/): ocultam avatares e etiquetas de nome dentro de uma região da cena.
* [Mover o jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/move-player/) e [Animações de avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/): controla o avatar do jogador.
* [Avatares NPC](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars/): mostra avatares que não são jogadores.
* [Obtenha dados do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/): lê dados sobre o jogador e todos os outros na cena.
* [Tipos de cor](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/): como construir `Color3` valores.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-nametags.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
