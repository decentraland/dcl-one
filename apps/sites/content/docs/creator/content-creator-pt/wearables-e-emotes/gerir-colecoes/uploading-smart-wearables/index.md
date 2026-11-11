> Diretrizes para enviar Smart Wearables para o Editor

# Enviando Smart Wearables

Depois de gerar o seu Smart Wearable usando o [SDK7](https://github.com/decentraland/docs/blob/main/creator/sdk7/projects/smart-wearables.md), o próximo passo é carregá-lo no builder. Este documento explica como carregar, publicar e colocar os seus Smart Wearables à venda.

## Carregar o seu ficheiro

Lembre-se de que precisa de criar uma collection antes de carregar o seu ficheiro. Se não souber como fazer isso, consulte [Criar uma coleção](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/manage-collections/creating-a-collection.md). Para carregar o seu Smart Wearable, arraste e solte o ficheiro na ***Novo item*** janela ou procure no seu computador. Será detetado automaticamente se o ficheiro for um Smart Wearable. **Lembre-se de que o tamanho máximo do ficheiro da coleção é 3 MB**.

![](/docs/_remote/48697c84cb446eeea05d89d3.webp)

Quando carregar o ficheiro, será pedido que carregue um vídeo de apresentação.

## Carregamento do seu vídeo de apresentação

Este vídeo de apresentação será exibido no Marketplace para que os compradores vejam como o Smart Wearable funciona.

Para carregar o seu vídeo de apresentação, arraste e solte um **.mp4** ficheiro de vídeo na ***Carregar um vídeo para o seu Smart Wearable*** janela ou procure no seu computador. **Lembre-se de que o tamanho máximo do ficheiro de vídeo é 4 MB e a duração máxima é de 15 segundos**.

![](/docs/_remote/763eb735dd9262151b2d4576.webp)

Quando carregar o vídeo de apresentação, será pedido que introduza um nome e defina a raridade e a categoria. Se quiser, também pode adicionar uma miniatura para o Smart Wearable.

### Raridade

Selecione a raridade do seu item.

![](/docs/_remote/5aa59b7826603f649de1aade.webp)

| Raridade    | Número de itens |
| ----------- | --------------- |
| Único       | 1               |
| Mítico      | 10              |
| Exótico     | 50              |
| Lendário    | 100             |
| Épico       | 1,000           |
| Raro        | 5,000           |
| Pouco comum | 10,000          |
| Comum       | 100,000         |

### Categoria

Os Wearables estão organizados em diferentes categorias, dependendo da parte de um Avatar que modificam. Nesta etapa, deve selecionar a categoria do item adequada.

![](/docs/_remote/805bd4e946bcf6044e5f79e7.webp)

### Miniaturas personalizadas

Pode adicionar a sua miniatura clicando no ícone da câmara e selecionando a imagem que pretende do seu computador. **Certifique-se de que a miniatura está num ficheiro .png quadrado de 256 px, com fundo transparente.** Tenha em atenção que o Curation Committee não aprovará coleções com miniaturas sem fundo transparente.

![](/docs/_remote/d45b65391e8cc81a9f4c7a8c.webp)


> [!WARNING]
> ⚠️ Ter um bom render do seu Wearable é crucial para o tornar mais apelativo para potenciais utilizadores do Marketplace. **É importante evitar adicionar gráficos para além do próprio Wearable, porque isso pode levar o Curation Committee a rejeitá-lo.**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### Atualizar vídeo de apresentação

Para atualizar o seu vídeo de apresentação, basta clicar no ícone da câmara e procurar o vídeo no seu computador. **Tenha em atenção que o tamanho máximo permitido do ficheiro é 4 MB e a duração não deve exceder 15 segundos**.

![](/docs/_remote/3375e0716ee39e30f5ccb5aa.webp)

### Propriedades

Ao lado da miniatura, vai encontrar as propriedades do seu wearable, como o número de triangles do seu modelo, o número de materials e as textures.

![](/docs/_remote/41d92d3af0ff7eebd17ea3cf.webp)

## Carregar Mouth, Eyes e Eyebrows

A categoria mouth, eyes e eyebrows tem um comportamento diferente no editor porque são apenas ficheiros .png. Para os carregar, basta arrastar e largar o ficheiro PNG como uma imagem transparente (256x256 píxeis). A mouth vai ser automaticamente tingida pela cor da pele, o mesmo acontecendo com as eyebrows, que serão tingidas pela cor do cabelo.


> [!WARNING]
> Se quiser que o asset seja mascarado, para que uma parte da mouth ou das eyebrows não seja afetada pela coloração, carregue um ficheiro zip com o PNG e os ficheiros de máscara. Lembre-se de que o ficheiro de máscara deve ter o sufixo "\_mask" para funcionar.


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

Depois de carregar os seus Wearables, acabará com um ecrã como este, que mostra os itens da sua coleção.

![](/docs/_remote/83043b56afafb5b81fe2be0f.webp)

## Definir o preço do seu Smart Wearable

Pode definir o preço do seu Wearable clicando em ***Definir preço***. Tudo isto pode ser editado a qualquer momento, por isso não se preocupe se quiser alterá-lo mais tarde. Os preços são definidos em MANA. Lembre-se de que, quando fizer mint de Wearables, eles são minted diretamente em Matic/Polygon. De facto, quando um utilizador compra o seu item, a transação será efetuada em Matic/Polygon MANA.

Também pode ***Torná-lo gratuito***, o que significa que o preço será 0 MANA e o endereço do beneficiário será null. Tenha em atenção que oferecer um produto gratuitamente como primary sale não impede que ele seja vendido por qualquer preço como secondary sale.

Não se esqueça de definir o endereço do beneficiário, que é o que receberá o MANA das suas vendas. Pode usar qualquer endereço Ethereum que quiser. Para preencher automaticamente o endereço com o qual tem sessão iniciada, clique em ***Eu sou o beneficiário.***

![](/docs/_remote/44c1580896b6f7d4ff6a5349.webp)

Guarde o preço e voltará à lista de Wearables na sua coleção. Quando clicar no item, obterá as informações gerais dele. Clique no botão ***Pré-visualizar no Editor*** para o ver no Editor.

![](/docs/_remote/8170f9434918babf3565876f.webp)

## O Editor

Depois de clicar em ***Pré-visualizar no Editor***, terá o Editor aberto. Pode editar as informações do seu Smart Wearable, incluindo a descrição, tags e overrides.

![](/docs/_remote/cabb347fa07def0f5e919e48.webp)

## Descrição

Esta é uma breve descrição do seu item que será apresentada no Marketplace.

## Overrides

Os overrides determinam que categorias de Wearable ou partes do corpo do avatar o seu item irá ocultar. Por exemplo, um chapéu com cabelo incorporado pode ter de ocultar a *Hair* categoria Hair. Um capacete de mergulhador de profundidade pode exigir ocultar acessórios da cabeça como brincos, Eyewear, tiaras, etc., que não ficariam visíveis. Podem ser selecionadas várias opções para cada override.

* **Base Body**: Isto refere-se às partes centrais do avatar, como a *head* e *hands*. Por exemplo, se estiver a criar um **Handwear** item, como uma mão mecânica de robô, provavelmente terá de ocultar *hands* para evitar sobreposição e clipping.
* **Wearables**: Isto inclui outras categorias de Wearable. Pode ocultar várias categorias. Consulte [**Criar Wearables**](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/wearables/creating-wearables.md) para mais detalhes sobre cada categoria e sobre como os itens interagem.


> [!WARNING]
> Nota: Os overrides que selecionar serão as definições predefinidas sugeridas para o seu Wearable. No entanto, os utilizadores podem personalizar quais os Wearables que ficam ocultos ou visíveis a partir da Backpack.


## Tags

As tags são palavras descritivas que os utilizadores podem usar em competições ou eventos!

## Pré-visualização

Para pré-visualizar o seu Smart Wearable, passe o rato sobre o elemento do wearable no canto superior esquerdo e clique no símbolo do olho.

![](/docs/_remote/38a5124d67833aaa252218c1.webp)

Ao clicar no ícone no canto inferior esquerdo, pode editar o Avatar. Isto é útil se tiver uma versão masculina e uma feminina do seu Wearable, por isso veja como ambas as versões ficam no Editor, testando diferentes emotes para identificar problemas de skinning e combinando outros Wearables para ver como se ajusta a outras roupas. Quando terminar de editar o seu wearable, clique em ***Save***.

![](/docs/_remote/2d3fb93f880674b537bb022c.webp)

## Testar no Decentraland

Mesmo depois de testar o Smart Wearable no Editor, é importante verificar como ele ficará e se comportará no Decentraland. Para o experimentar lá, vá ao separador Collections. Selecione a coleção pretendida e clique no botão ***Ver no Decentraland***.

![](/docs/_remote/f98021fb76ec1a330e1a9f4a.webp)

Depois de clicar no ***Ver no Decentraland*** botão, aparecerá a seguinte janela pop-up. Ao selecionar ***Empty Parcels*** irá teletransportá-lo para um local com pouco conteúdo, que carregará mais rapidamente. Selecionar ***Genesis Plaza*** levá-lo-á para a praça principal.

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

Assim que selecionar o local para teleportar, abrir-se-á um novo separador no seu browser e receberá esta mensagem.

![](/docs/_remote/61eed76d8ed92cce5d514ea9.webp)

Clique em ***Mudar para Sepolia***, e aparecerá uma janela pop-up da sua wallet a pedir-lhe para mudar a network. Clique em Switch Network, e o novo separador será atualizado automaticamente. Para testar o seu Smart Wearable, pode ir à backpack e selecioná-lo.


> [!WARNING]
> Se ainda não tiver ativado as Ethereum testnet networks, pode seguir este [Artigo da Metamask](https://support.metamask.io/hc/en-us/articles/13946422437147-How-to-view-testnets-in-MetaMask).


![](/docs/_remote/91716ea1b880be3f761e1a9c.webp)

## Antes de publicar

Certifique-se de definir o preço corretamente, adicionar uma boa descrição e verificar duas vezes se todas as informações e definições estão corretas. Se tiver preenchido todas as informações necessárias, verá ***Concluído*** como estado do seu item.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/gerir-colecoes/uploading-smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
