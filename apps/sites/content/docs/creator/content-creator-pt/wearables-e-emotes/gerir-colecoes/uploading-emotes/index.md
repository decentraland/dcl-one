> Diretrizes para enviar Emotes para o Editor

# Enviando Emotes

Assim que exportar o seu emote, terá de o carregar para o Builder. Este documento vai cobrir o processo de carregamento de emotes.

## **Carregar o seu ficheiro**

Lembre-se de que precisa criar uma coleção antes de poder carregar o seu ficheiro. Se não souber como fazer isso, consulte [Criar uma coleção](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). Para carregar o seu emote, basta arrastar e largar o ficheiro na ***Novo item*** janela ou procurar no seu computador. Ele detetará automaticamente se o ficheiro tem alguma animação, identificando-o como um emote.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

Arraste e largue o seu ficheiro de animação para o carregar.

Ser-lhe-á pedido que introduza um nome para o seu emote, defina a sua raridade, a categoria e o modo de reprodução. Por baixo da miniatura é mostrado o número de clipes de animação no seu ficheiro, a duração da animação em segundos, a duração em frames e a taxa de fotogramas.

![](/docs/_remote/e376a1e332918af541d1a910.webp)

## **Carregar Emotes Usando um Ficheiro .zip**

Se o emote tiver som (*mp3* ou *ogg*) tem de ser compactado com o `.glb`. Depois disso, basta arrastar e largar o `.zip` para o Builder. Além disso, é possível adicionar um `.json` ficheiro juntamente com os outros assets no mesmo `.zip` para adicionar nome, descrição, raridade, categoria, modo de reprodução e/ou tags. Estas são as definições de cada um:

* `nome da scene`: Nome do Emote
* `descrição`: Descrição do seu Emote (no total, não mais de 64 caracteres, contando os espaços)
* `category`: Categoria do Emote ("dance", "stunt", "greetings", "fun", "poses", "reactions", "horror", "miscellaneous")
* `rarity`: Raridade do Item ("unique", "mythic", "legendary", "epic", "rare", "exotic", "uncommon", "common")
* `play_mode`: Animação simples ou em loop ("simple", "loop")
* `tags`: Tags para serem facilmente encontradas no Marketplace.

Para adicionar essas definições ao emote, crie apenas um ficheiro de texto, nomeando-o **emote.json** e adicione as linhas seguintes, como no exemplo:

```
{
  "name": "Pancada de Ténis",
  "description": "Mostra-me que sabes jogar ténis",
  "category": "fun",
  "rarity":"epic",
  "play_mode": "simple",
  "tags":["ténis", "emote", "pancada"]

}

```

Desta forma, o Builder vai tomar todas as informações do .json e aplicá-las automaticamente ao emote.

**Raridade**

Consulte a tabela de raridade em [aqui](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/#rarity).

## **Categoria**

Escolha a categoria que melhor descreve o seu emote.

* Dança
* Acrobacia
* Saudações
* Diversão
* Poses
* Reações
* Terror
* Diversos

## **Modo de Reprodução**

Atualmente existem dois modos de reprodução:

* ***Reproduzir uma vez***: significa que o seu emote só será reproduzido uma vez. Depois disso, o avatar voltará para *Idle* posição.
* ***Loop***: o emote continuará a ser reproduzido em loop até o utilizador introduzir outra ação.

## **Miniaturas personalizadas**

Para emotes, não precisa de carregar quaisquer imagens, uma vez que o editor já tem uma ferramenta incorporada para criar uma miniatura. Basta selecionar o fotograma que melhor representa a ação.

As pessoas devem conseguir identificar sobre o que é a animação através da miniatura. Se uma imagem frontal não for suficiente, tente rodar o modelo, aproximar ou afastar o zoom, deslocar para cima ou para baixo, ou escolher qualquer fotograma do seu clipe. É mesmo importante que selecione a melhor imagem!

![](/docs/_remote/9efbcbc5b36b6a3edfbcf38b.webp)

Rode, aproxime ou afaste o zoom, desloque para cima e para baixo. Use as ferramentas para obter a melhor imagem da sua animação!

## O Editor

Depois de definir a miniatura, o editor será aberto. É aqui que pode verificar se a animação está a ser reproduzida corretamente, uma vez que um avatar 3D a estará a executar.

![](/docs/_remote/58fef5ae57e3864b91e06697.webp)

Ao clicar no ícone no canto inferior esquerdo, poderá editar o avatar. Clique no ícone do cilindro no canto inferior direito para verificar se a sua animação permanece dentro dos limites de altura e espaço.

![](/docs/_remote/c32c351247d541f21bafadb2.webp)

O ícone no canto inferior esquerdo permite editar o avatar.

![](/docs/_remote/f4f828d6e5577c46357b79cf.webp)

O ícone do cilindro mostra os limites da animação.

Também pode editar o nome, a miniatura, a categoria, a raridade e o modo de reprodução do emote, bem como adicionar uma descrição da animação e tags. Clique em ***Save*** quando terminar.

* **Descrição:** Esta é uma breve descrição do seu item que será apresentada no Marketplace.
* **Utilidade** Descreva a utilidade no mundo do Wearable ou Emote.
* **Tags:** As tags são simplesmente palavras descritivas que os utilizadores podem usar ao procurar ou filtrar itens. Estas são relevantes para competições ou eventos!

![](/docs/_remote/ae584da404403aeb0e47bcee.webp)

## Testar no World

Mesmo depois de testar a animação no editor, é importante verificar como ela vai realmente parecer e comportar-se em Decentraland. Para a testar no mundo, vá ao separador Collections, selecione a coleção desejada e serão mostrados todos os itens que ela contém. Clique em Ver no Decentraland.

\
\\\\

![](/docs/_remote/7fc9c8d40af1b921993b8bfb.webp)

\
\\\\


> [!NOTE]
> Ser-lhe-á pedido que escolha se quer testá-lo numa Empty Parcel ou na Genesis Plaza. Selecionar Empty Parcels levá-lo-á para um local com pouco conteúdo, que carregará mais depressa. Selecionar Genesis Plaza levá-lo-á para a praça principal da Genesis Plaza.


\
\\\\

![](/docs/_remote/d1c728c7c0ffee7fe42730da.webp)

\
\\\\


> [!NOTE]
> Depois de escolher, será aberta uma nova aba no seu browser com uma mensagem a informar que está prestes a usar um Catalyst personalizado. Clique em TRUST PEER-TESTING.DECENTRALAND.ORG para continuar.


\
\\\\

![](/docs/_remote/5e0eef48997d27eadb27149b.webp)

\
\\\\


> [!NOTE]
> Depois disso, receberá um pop-up a pedir para abrir o Decentraland Explorer. Assim que o Explorer estiver a correr, clique em Jump Into Decentraland.


\
\\\\

![](/docs/_remote/0518f3adcaa8974ae07124e4.webp)

\
\\\\


> [!NOTE]
> Uma vez no mundo, prima I para abrir a mochila e, no separador dos emotes, selecione o emote para testar.


\
\\\\

![](/docs/_remote/80aeb1e924a01a378e86f206.webp)

\
\\\\

## **Antes de publicar**

Certifique-se de adicionar uma boa descrição e verificar se todas as informações e definições estão corretas. Verifique também a miniatura. Se tiver preenchido todas as informações necessárias, verá Pronto para Enviar como o estado do seu item. Agora o seu emote está pronto para ser publicado!

![](/docs/_remote/e404c18c57fe014dd11e6091.webp)

O estado Pronto para Enviar significa que o seu ficheiro está pronto para ser publicado!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/gerir-colecoes/uploading-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
