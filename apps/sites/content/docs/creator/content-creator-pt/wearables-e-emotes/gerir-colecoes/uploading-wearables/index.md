> Diretrizes para enviar Wearables para o Editor

# Enviando Wearables

Depois de exportar o seu Wearable, terá de o carregar para o Builder. Este documento irá cobrir o processo de carregamento de wearables.

## Carregar o seu ficheiro

Lembre-se de que precisa criar uma coleção antes de poder carregar o seu ficheiro. Se não souber como fazer isso, consulte [Criar uma coleção](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). Para carregar o seu Wearable, basta arrastar e largar o ficheiro na ***Novo item*** janela ou navegar no seu computador. Será detetado automaticamente se o ficheiro é um Emote ou um Wearable. **Lembre-se de que o tamanho máximo do ficheiro da coleção é 3 MB**.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

Quando carregar o ficheiro, ser-lhe-á pedido que selecione uma body shape, introduza um nome e defina a raridade e a categoria. Também pode adicionar a miniatura do Wearable.

### **Body Shape**

Para garantir que os seus wearables possam ser usados pelos avatares pretendidos, tem de carregar ficheiros GLB separados para cada body shape. Se tiver duas versões separadas do wearable, uma masculina e outra feminina, pode adicionar uma das representações durante o processo de carregamento e depois adicionar a outra mais tarde usando o editor. Se o seu wearable for unissexo, certifique-se de carregar um único ficheiro GLB concebido para se adaptar às versões masculina e feminina.

![](/docs/_remote/9c4ad1dbfd25f7f986e2b0d5.webp)

### **Raridade**

Selecione a raridade do seu item.

![](/docs/_remote/a57649e01874c1a4789f0456.webp)

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

### **Categoria**

Os wearables estão organizados em diferentes categorias, dependendo da parte de um avatar que modificam. Selecione a categoria apropriada para o seu item.

![](/docs/_remote/ac233f1f8c3b34463031b787.webp)

### **Miniaturas personalizadas**

Pode adicionar a sua própria miniatura personalizada clicando no ícone da câmara e navegando no seu computador. **A miniatura deve ser um ficheiro .png quadrado de 256 px com fundo transparente.** As coleções que contenham miniaturas sem fundos transparentes não serão aceites pelo Comité de Curadoria.

![](/docs/_remote/5a563c26949da7b96f8c2028.webp)


> [!WARNING]
> ⚠️ Ter uma boa renderização do seu Wearable é crucial para o tornar mais apelativo para potenciais utilizadores no Marketplace. **É importante evitar adicionar quaisquer gráficos que não sejam o próprio wearable, porque isso pode levar o comité de curadoria a rejeitá-lo.**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### **Spring Bones**

Se o seu Wearable contiver Bones com `springbone` no nome, o Builder irá detetá-los automaticamente e mostrar um **Spring Bones** painel de configuração. A partir deste painel pode ajustar parâmetros físicos como rigidez, gravidade e arrasto para cada cadeia de spring bone. Consulte [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) para mais detalhes sobre como configurar spring bones no seu modelo.

### **Propriedades**

Por baixo da miniatura, encontrará as propriedades do seu Wearable, o número de triângulos do seu modelo, o número de materiais e texturas.

![](/docs/_remote/c1c0778d6b438f3e33e8c6cf.webp)

## **Carregar Mouth, Eyes e Eyebrows**

A categoria mouth, eyes e eyebrows tem um comportamento diferente no editor porque são apenas ficheiros .png. Para os carregar, basta arrastar e largar o ficheiro PNG como uma imagem transparente (256x256 píxeis). A mouth vai ser automaticamente tingida pela cor da pele, o mesmo acontecendo com as eyebrows, que serão tingidas pela cor do cabelo.


> [!WARNING]
> Se quiser que o asset seja mascarado, para que uma parte da mouth ou das eyebrows não seja afetada pela coloração, carregue um ficheiro zip com o PNG e os ficheiros de máscara. Lembre-se de que o ficheiro de máscara deve ter o sufixo "\_mask" para funcionar.


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

Depois de carregar os seus wearables, ficará com um ecrã como este, que mostra os itens da sua coleção.

![](/docs/_remote/b1d31fcc4dddce1716af7bc8.webp)

## **Definir o preço dos seus wearables**

Depois de publicar a sua coleção e esta ser aprovada, poderá então ativar as vendas na sua coleção.

![](/docs/_remote/dd9d2d46268767048dd15f53.webp)

Pode definir o preço do seu wearable clicando em ***Colocar à venda***. Tudo isto pode ser editado a qualquer momento, por isso não se preocupe se quiser alterá-lo mais tarde. Os preços são definidos em MANA. Lembre-se de que, quando faz mint de wearables, estes são feitos diretamente em Matic/Polygon. Quando um utilizador compra o seu item, a transação será realizada em Matic/Polygon MANA.

Também pode ***Torná-lo gratuito***, o que significa que o preço será definido como 0 MANA e o endereço do beneficiário será nulo. Saiba que torná-lo gratuito (venda primária) não impede que seja vendido por qualquer preço numa venda secundária.

Não se esqueça de definir o endereço do beneficiário, que é o que receberá o MANA das suas vendas. Pode usar qualquer endereço Ethereum que quiser. Para preencher automaticamente o endereço com o qual tem sessão iniciada, clique em ***Eu sou o beneficiário.***

![](/docs/_remote/f25c9cc68be82373f6fe392e.webp)

Coloque o item à venda e voltará à lista de wearables da sua coleção. Quando clicar no item, verá as suas informações gerais. Clique no botão ***Pré-visualização*** para o ver no editor.

![](/docs/_remote/528700c2d810068ed39ccdb2.webp)

## O Editor

Depois de clicar em ***Pré-visualização***, o editor ficará aberto. Pode editar todas as informações do seu Wearable, bem como adicionar novas, como descrição, tags e overrides. É também aqui que adiciona outra representação de body shape ao seu Wearable.

![](/docs/_remote/c21cb9360544651f12054903.webp)

## **Descrição**

Esta é uma breve descrição do seu item que será apresentada no Marketplace.

## **Overrides**

Os overrides determinam que categorias de Wearable ou partes do corpo do avatar o seu item irá ocultar. Por exemplo, um chapéu com cabelo incorporado pode ter de ocultar a *Hair* categoria Hair. Um capacete de mergulhador de profundidade pode exigir ocultar acessórios da cabeça como brincos, Eyewear, tiaras, etc., que não ficariam visíveis. Podem ser selecionadas várias opções para cada override.

* **Base Body**: Isto refere-se às partes centrais do avatar, como a *head* e *hands*. Por exemplo, se estiver a criar um **Handwear** item, como uma mão mecânica de robô, provavelmente terá de ocultar *hands* para evitar sobreposição e clipping.
* **Wearables**: Isto inclui outras categorias de Wearable. Pode ocultar várias categorias. Para mais detalhes sobre cada categoria e sobre como os itens interagem, consulte [**Criar Wearables**](/docs/creator/wearables-and-emotes/wearables/creating-wearables/).


> [!WARNING]
> Nota: Os overrides que selecionar serão as definições predefinidas sugeridas para o seu Wearable. No entanto, os utilizadores podem personalizar quais os Wearables que ficam ocultos ou visíveis a partir da Backpack.


## **Tags**

As tags são simplesmente palavras descritivas que os utilizadores podem usar ao procurar ou filtrar itens. Estas são relevantes para competições ou eventos!

## **Permissão de exportação VRM**

Quando esta propriedade estiver ativada, permitirá aos proprietários do seu item incluí-lo em VRM Avatar Exports, para que o possam mostrar fora de Decentraland.

## **Outline**

O Toggle Outline controla se o seu Wearable é compatível com o sistema de renderização de outline do Decentraland (usado nas definições de qualidade Média/Alta). Por predefinição, isto está ativado para todos os novos wearables.

Deve desativar esta opção se o seu wearable:

* Tem outlines criados manualmente no modelo
* Usa normals invertidas
* Mostra artefactos visuais quando o efeito de outline é aplicado

A maioria dos wearables funciona bem com a definição predefinida (ativada). Desative-a apenas se notar problemas visuais relacionados com outline no seu wearable. Depois de desativar o outline, terá de guardar as alterações clicando em ***Guardar***.

![](/docs/_remote/69f438abb59b36e0dc7bcc84.webp)

## **Adicionar outra representação**

Se o seu wearable tiver uma representação diferente para masculino e feminino, terá de carregar outro ficheiro. Até agora, só tem um carregado. No exemplo, era a versão feminina do Krampus Sweater. Para adicionar a outra representação, clique nos três pontos (*…*) no canto superior direito, junto a ***Propriedades*** e selecione ***Add male/female*** representação. No exemplo abaixo, precisávamos da versão masculina.

![](/docs/_remote/21001d89e30650778f2bd9b8.webp)

Depois de clicar nele, verá a janela Add Male/Female representation; arraste e largue o outro ficheiro de representação para o carregar. Depois de carregado, aparecerá outra janela com o wearable e, se estiver tudo bem, basta clicar em Save.

![](/docs/_remote/983512d9a30c68af5edb426d.webp) ![](/docs/_remote/7debe500c2fea74349f7fa23.webp)

## **Pré-visualização**

Para pré-visualizar o seu wearable, passe o rato sobre o ícone do wearable no canto superior esquerdo e clique no símbolo do olho.

![](/docs/_remote/4634ace580b72aed8b781aed.webp)

Ao clicar no ícone no canto inferior esquerdo, poderá editar o avatar. Isto é bastante útil se tiver uma versão masculina e feminina do seu wearable, por isso certifique-se de verificar como ambas as versões ficam no editor, testando diferentes emotes para identificar se existem problemas de skinning e combinando outros wearables para ver como ficam com roupas diferentes. Quando terminar de editar o seu wearable, clique em ***Guardar***.

![](/docs/_remote/cec3d31a2ab318de454d52a0.webp)

## **Testar no World**

Mesmo depois de testar o wearable no editor, é importante verificar como ele realmente vai ficar e comportar-se em Decentraland. Para o testar no World, vá ao separador Collections. Selecione a coleção desejada e clique no botão ***Ver no World***.

![](/docs/_remote/4a233b404952959b105fa854.webp)

Depois de clicar, o seguinte pop-up irá aparecer. Selecionar ***Empty Parcels*** irá teletransportá-lo para um local com pouco conteúdo, que carregará mais rapidamente. Selecionar ***Genesis Plaza*** levá-lo-á para a praça principal.

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

Depois de selecionar See in World, será aberta um novo separador no seu navegador e receberá esta mensagem.

![](/docs/_remote/a95471fb4beb838e7547a9bf.webp)

Clique em ***TRUST PEER-TESTING.DECENTRALAND.ORG*** e aparecerá um pop-up. Basta clicar em Open Decentraland. Para testar o seu wearable, vá à Backpack e equipe-o.

![](/docs/_remote/bd6c470a523472534ab4282f.webp)

## **Antes de publicar**

Certifique-se de definir o preço corretamente, adicionar uma boa descrição e verificar duas vezes se todas as informações e definições estão corretas. Se tiver preenchido todas as informações necessárias, verá ***Concluído*** como estado do seu item.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/gerir-colecoes/uploading-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
