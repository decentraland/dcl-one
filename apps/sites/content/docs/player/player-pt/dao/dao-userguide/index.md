> Como usar a DAO da Decentraland

# Guia do usuário da DAO

## Índice

* [Fazendo login](#logging-in)
* [Poder de voto](#voting-power)
* [Condições de Aprovação/Rejeição](#approvereject-conditions)
* [Navegando pelas propostas](#browsing-all-proposals)
* [Visualizando uma proposta](#viewing-a-proposal)
* [Adicionando propostas à sua lista de acompanhamento](#adding-proposals-to-your-watch-list)
* [Votação](#voting)
* [Participando no Fórum](#participating-in-the-forum-discussions)
* [Criando uma proposta](#creating-a-proposal)
* [Categorias de propostas](#proposal-categories)
* [Excluindo uma proposta](#deleting-a-proposal)

## Fazendo login

Para começar com a DAO, visite [governance.decentraland.org](https://governance.decentraland.org). Você será apresentado a uma tela de boas-vindas e a um tutorial de início rápido.

Depois de ler o tutorial de início rápido, clique em **Entrar**. Você deve conectar uma wallet para usar a DAO. Atualmente, você pode usar **Metamask** ou **Fortmatic**. Certifique-se de que a wallet que você está usando contém os tokens relevantes para participar da DAO (MANA, NAMES ou LAND)

Após conectar sua wallet, você será levado à página inicial da DAO, que apresenta uma lista de todas as propostas atualmente ativas.

## Poder de voto

O poder de voto é calculado a partir do saldo total de MANA, NAMES e LAND associado à wallet conectada à DAO. **A DAO considera tanto os seus saldos wrapped quanto unwrapped, então você não precisa fazer unwrap de nenhum MANA para obter todo o seu poder de voto.** No entanto, você pode fazer unwrap de qualquer MANA wrapped anteriormente a qualquer momento na aba "Voting Power". Ainda há uma taxa de gas associada ao unwrap de MANA, pois isso é uma transação na mainnet da Eth.

Seu voto na DAO da Decentraland é ponderado de acordo com o saldo de MANA, NAMES e LAND associado à conta com a qual você faz login.

Para ver o seu poder de voto atual, navegue até a aba **Poder de Voto** .

#### Como o poder de voto é calculado?

O poder de voto é representado por **"VP"**. MANA, NAME, LAND e Legacy Wearables contribuem para o seu poder de voto total da seguinte forma:

* 1 MANA contribui com 1 VP
* 1 NAME contribui com 100 VP
* 1 parcela de LAND contribui com 2000 VP
* As seguintes Legacy Wearables Collections contribuem com 1 VP para cada **incomum**, 5 VP para cada **raro**, 10 VP para cada **épico**, 100 VP para cada **lendário**, e 1000 VP para cada **mítico**:
  * [Community Contest Collection](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
  * [DCL Public Explorer Launch Collection](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
  * [Exclusive Masks Collection](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
  * [Halloween 2019 Collection](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
  * [My Crypto Heroes Collection](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
  * [Xmass 2019 Collection](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* Cada Estate vale 2000 multiplicado pelo número de parcelas individuais de LAND nesse Estate. Por exemplo, um Estate com 2 parcelas contribuirá com 4000 VP para o seu poder de voto total.

#### O que acontece se o seu poder de voto mudar antes de uma proposta ser encerrada?

Se o seu saldo de MANA ou LAND mudar, isso afetará o seu poder de voto, mas apenas nas propostas criadas depois que o seu saldo mudar.

No momento em que uma nova proposta é criada, a DAO considera os saldos de MANA e LAND de todos os votantes para calcular o poder de voto. Em outras palavras, quando você vota em uma proposta, o poder de voto do seu voto será igual ao saldo que você tinha no instante em que a proposta foi inicialmente criada. Você pode votar com esse VP e depois alterar seu saldo de MANA/LAND sem afetar o peso do seu voto.

#### Por que a DAO da Decentraland usa votos ponderados?

Se a DAO da Decentraland desse a cada endereço Ethereum um voto, e cada voto tivesse o mesmo peso, os usuários poderiam criar quantos endereços separados quisessem para obter mais poder de voto.

Determinar o poder de voto considerando os saldos de MANA, NAME e LAND é atualmente a maneira mais segura de limitar a quantidade de influência que cada votante pode ter. Além disso, quanto mais MANA, NAME ou LAND você possuir na Decentraland, maior será a sua participação pessoal considerada, dando assim mais influência ao seu voto dentro da DAO.

## Condições de Aprovação/Rejeição

Dependendo do tipo de proposta, uma certa quantidade de VP participante é necessária para considerar a votação válida. Você verá isso referido na dApp de Governança como o **Limiar de Aceitação** da proposta. Assim que o limiar de aceitação for atingido, uma proposta é aprovada se o poder de voto total a favor da proposta for maior que o poder de voto total contra a proposta. Este é o modelo típico de maioria 50/50 que esperamos ver na maioria dos votos democráticos.

O único tipo de proposta que não segue essa regra são as **Enquetes de pré-proposta**. Como essas propostas são consideradas um mecanismo não vinculante para reunir o feedback da comunidade sobre uma ideia, elas não têm opções predefinidas de Sim/Não para votação. Os usuários podem adicionar mais de duas opções, tornando-a uma enquete de múltipla escolha. É importante mencionar que, se as enquetes de pré-proposta atingirem o Limiar de Aceitação definido de 500k VP, elas podem ser promovidas a uma **proposta de rascunho** e acabar se tornando uma **proposta de Governança**.

## Navegando por todas as propostas

Para ver todas as propostas na DAO, visite [governance.decentraland.org](https://governance.decentraland.org). A página inicial da DAO lista todas as propostas adicionadas recentemente, ordenadas da mais recente para a mais antiga por padrão.

Você pode filtrar propostas por **Resultados**:

* **Ativas** – propostas que estão atualmente em votação
* **Aprovadas** – propostas que foram aprovadas pela comunidade
* **Rejeitadas** – propostas que já foram votadas e que foram rejeitadas pela comunidade ou não atingiram o limiar de aceitação
* **Implementadas** – propostas que foram implementadas on-chain pelo Comitê da DAO
* **Concluídas** – propostas que estão encerradas, mas não têm resultados de sim/não, como enquetes de múltipla escolha

Você também pode filtrar propostas por **Categoria** usando a coluna de categoria no lado esquerdo da UI:

* **Todas as propostas** – exibe todas as propostas, independentemente da categoria ou do resultado
* **Catalyst Node** – exibe apenas propostas para adicionar novos nós Catalyst
* **Ponto de interesse** – exibe apenas propostas para adicionar novos POIs
* **Banimento de nome** – exibe apenas propostas para banir um nome
* **Solicitação de subsídio** – exibe apenas solicitações de subsídio
* **Enquete de pré-proposta** – exibe apenas enquetes não vinculantes
* **proposta de rascunho** – exibe apenas propostas de rascunho
* **proposta de Governança** – exibe apenas propostas finais vinculantes de Governança

Para ver apenas as propostas que foram aprovadas, clique na **Implementadas** aba. As propostas implementadas nesta aba são ordenadas da mais recente para a mais antiga por padrão.

## Visualizando uma proposta

Para ler uma proposta, basta clicar no título da proposta para ver a página de detalhes.

Cada página de detalhes da proposta inclui todas as informações descritivas fornecidas pela pessoa que submeteu a proposta.

Você também encontrará links para o tópico de discussão da proposta no Fórum, botões para adicionar uma proposta à sua lista de acompanhamento, os resultados atuais da votação, botões para votar, o limiar de aceitação, seu Poder de Voto atual e quem votou na proposta.

As páginas de detalhes da proposta também listam o nome único do avatar associado ao endereço Ethereum que abriu a proposta, se houver, e as datas de início e fim da proposta.

Por fim, você verá um link para a entrada da proposta no Snapshot – a plataforma de votação usada pela DAO da Decentraland.

## Adicionando propostas à sua lista de acompanhamento

Para adicionar uma proposta à sua lista de acompanhamento, veja a página de detalhes da proposta e clique em **Adicionar à minha lista de acompanhamento**.

Para remover uma proposta da sua lista de acompanhamento, clique em **Remover da minha lista de acompanhamento** na página de detalhes da proposta. Você também pode clicar no ícone de bandeira vermelha em qualquer proposta atualmente na sua lista de acompanhamento para removê-la.

## Participando das discussões no Fórum

Como uma plataforma de Governança, a DAO da Decentraland é mais eficaz quando acompanhada de discussões frequentes dentro da comunidade. Toda vez que uma nova proposta é aberta em **governance.decentraland.org**, um tópico correspondente é aberto automaticamente em [**forum.decentraland.org**](https://forum.decentraland.org).

Antes de votar, reserve um tempo para ver e participar dessas discussões no fórum. Clique no botão **Discutir no fórum** na página de detalhes de qualquer proposta da DAO que você queira discutir. Você também pode navegar pelos tópicos abertos acessando [forum.decentraland.org](https://forum.decentraland.org), indo para a **Governança** categoria e navegando pelos tópicos abertos.

Você não precisa possuir tokens para participar dessas discussões! Todos são bem-vindos para contribuir com a conversa.

## Votação

Para votar em uma proposta, faça login na DAO em [governance.decentraland.org](https://governance.decentraland.org) com uma wallet que contenha MANA, NAME ou LAND.


> [!NOTE]
> **Saldo mínimo necessário para votar:** Somente detentores de MANA, NAME ou LAND podem votar em propostas na DAO da Decentraland. **O saldo mínimo atual necessário para ter um voto ponderado nas propostas da DAO é 1VP** Votar com saldo zero resultará em um voto com peso 0, o que não afeta os resultados finais.


Para votar em uma proposta depois de conectar sua wallet, basta ver a página de detalhes da proposta e clicar no **VOTE YES** ou **VOTE NO** botão, ou selecione uma das opções de múltipla escolha se for uma enquete, de acordo com como você gostaria de votar.

Certifique-se de ler a proposta completa para entender o assunto em discussão e o que acontecerá se a proposta for aprovada ou rejeitada.

Depois de clicar no botão Votar, sua wallet Ethereum conectada solicitará que você assine a transação. Lembre-se, trata-se apenas de uma transação assinada e não requer taxa de gas.

Você terá a opção de adicionar a proposta à sua lista de acompanhamento; essa é uma boa maneira de acompanhar as propostas de seu interesse. Se você não quiser adicionar a proposta à sua Watchlist, basta clicar em **Não, obrigado**.

Depois de enviar seu voto, você pode alterá-lo a qualquer momento até o final do período de votação, conforme mostrado na página de detalhes da proposta.

#### O que acontece se o seu VP mudar antes de uma proposta ser concluída?

A DAO calcula seu poder de voto para cada proposta individual no momento em que cada proposta é criada. Se o seu VP mudar após esse momento, isso não afetará seu voto nessa proposta.

Para uma discussão completa sobre o poder de voto e como e quando ele é calculado, consulte a [Poder de Voto](#voting-power) seção do Guia do Usuário.

## Criando uma proposta

Para criar uma nova proposta na DAO da Decentraland, comece fazendo login em [governance.decentraland.org](https://governance.decentraland.org) e conectando uma wallet que contenha MANA, NAME ou LAND.

Depois de fazer login e conectar sua wallet, clique em **Enviar proposta** e selecione a categoria de proposta que deseja usar. Cada categoria fornecerá um formulário que permite inserir as informações relevantes para sua proposta.

O formulário de cada categoria é diferente, então certifique-se de selecionar a categoria correta para sua proposta. Os formulários de proposta na DAO suportam Markdown, então você pode formatar o conteúdo da sua proposta para torná-la mais legível. Isso é especialmente útil para propostas mais longas.

Para visualizar seu texto Markdown renderizado, alterne o botão **Preview** . Se você não estiver familiarizado com Markdown, pode usar texto simples.

Algumas categorias de propostas têm limiares mínimos de submissão em VP, o que significa que você precisa ter pelo menos uma certa quantidade de VP para submeter as propostas. Isso será informado a você na dApp de Governança.

Depois de concluir o formulário da proposta para a categoria selecionada, clique em **Enviar proposta**. Após enviar sua proposta com sucesso, você será levado à página de detalhes da nova proposta, onde poderá adicioná-la à sua lista de acompanhamento e votar.

## Categorias de propostas

### Propostas vinculantes

#### 📍 Ponto de interesse

Pontos de interesse são locais notáveis em Decentraland. Esses "POIs" são destacados em várias áreas da UI do mundo virtual e são promovidos como bons lugares para os usuários explorarem, especialmente pessoas novas na Decentraland.

Se você criou um local, ou encontrou um local que acha que deveria estar nesta lista, ou se acha que um POI atual deveria ser removido por não ser mais interessante, você pode usar esta categoria de proposta para apresentar sua sugestão à DAO.

#### 🚫 Banimento de nome

A lista de "nomes banidos" inclui nomes ofensivos ou prejudiciais de avatar e localização que não são permitidos na Decentraland. Quaisquer nomes nesta lista não podem ser reivindicados, usados ou transferidos entre usuários. Para sugerir o banimento de um nome, você pode usar a categoria de proposta Banimento de nome na DAO.

#### 🌐 Nós Catalyst

Os nós Catalyst são os servidores operados pela comunidade que fornecem o conteúdo e estabelecem as conexões peer-to-peer necessárias para manter o mundo virtual da Decentraland em funcionamento. Sempre que um usuário abre a Decentraland, ele se conecta a um desses nós. No entanto, apenas os nós aprovados pela DAO são usados na rede da Decentraland.

Para sugerir a adição de um novo nó à rede, você pode usar a categoria Catalyst Node.

### Propostas do Processo de Governança

Algumas propostas não são tão simples quanto adicionar ou remover um item de uma lista; elas exigem sinalização da comunidade, discussões e caminhos de implementação. Essas propostas devem ser submetidas por meio de um processo de Governança de três etapas que começa com uma enquete e termina com uma proposta vinculante.

O processo de votação inclui três etapas: uma Enquete de Pré-Proposta, uma Proposta de Rascunho e uma Proposta de Governança. Cada nível terá limiares de submissão e aprovação progressivamente maiores para garantir que decisões importantes de Governança sejam tomadas por uma maioria representativa (com base no Poder de Voto). Cada etapa deve atingir o limiar de VP definido para ser promovida à próxima.

#### 📊 Enquete de pré-proposta

Esta é a primeira etapa para chegar à Proposta final vinculante de Governança. As enquetes na DAO da Decentraland são questionários não vinculantes de múltipla escolha que podem ser usados para medir a opinião geral ou o sentimento da comunidade em relação a diferentes मुद्दos. Elas são não vinculantes porque a DAO não age automaticamente com base nos resultados de nenhuma dessas enquetes. Se uma enquete de pré-proposta reunir VP participante suficiente (500k VP é o limiar de aceitação), ela pode ser promovida a uma Proposta de Rascunho.

#### 📊 Proposta de Rascunho

Uma Proposta de Rascunho apresenta uma possível política à comunidade em um formato estruturado e formaliza a discussão sobre os possíveis impactos da proposta e os caminhos de implementação. Uma Proposta de Rascunho que falhe ou não atinja esse limiar pode ser alterada e reenviada uma vez. Se uma Proposta de Rascunho reunir VP participante suficiente (1M VP é o limiar de aceitação), ela pode ser promovida a uma proposta vinculante de Governança.

#### 📊 Proposta de Governança

Esta é a última etapa do processo de Governança e é a única que é vinculante. Esta proposta deve detalhar todas as informações, dados, métodos, avaliações ou qualquer outra informação relevante para implementar esta proposta. Uma Proposta de Governança é aceita se atingir o limiar de aceitação de 6M VP e a opção Sim obtiver maioria simples.

## Excluindo uma proposta

As propostas só podem ser excluídas pela pessoa que as criou.

Para excluir uma de suas propostas, navegue até a página de detalhes da proposta que deseja excluir. Clique em **Excluir proposta** na parte inferior da coluna da direita. Você terá que confirmar a exclusão.

Cuidado! Propostas excluídas não podem ser restauradas!

Se você excluir uma proposta depois que as pessoas votaram nela, nada acontecerá. Propostas excluídas com ações vinculantes não serão implementadas pelo Comitê da DAO.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/dao/dao-userguide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
