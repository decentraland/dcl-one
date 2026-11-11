> Configure uma campanha do Decentraland Rewards: crie-a, adicione estoque de wearable ou emote, gerencie fundos e ative dispensers.

# Primeiros Passos

Esta página orientará você no processo de uso do sistema Rewards para incentivar o engajamento dos usuários com Wearables e Emotes do Decentraland. Você aprenderá como configurar e gerenciar uma campanha de recompensas, incluindo criar uma campanha, adicionar itens ao estoque da campanha, gerenciar fundos e usar dispensers para entregar recompensas. Além disso, encontrará instruções para ativar sua campanha e lidar com taxas de transação.

## Pré-requisitos

Antes de começar a usar o sistema Rewards, certifique-se de que você tem uma coleção [aprovada e pronta para minting](/docs/creator/content-creator-pt/wearables-e-emotes/publicacao-de-colecoes/publishing-collections/). Esta coleção deve conter os itens que você planeja oferecer como recompensas em sua campanha.

## Crie sua primeira campanha

Uma campanha refere-se a uma iniciativa de incentivo projetada para aumentar o engajamento dos usuários por meio de ofertas de recompensas. Nesta seção, você descobrirá os detalhes de configuração da campanha, juntamente com os pré-requisitos para iniciar a distribuição de recompensas.

1. Acesse o [dApp Rewards](https://decentraland.org/rewards)
2. Conecte sua Wallet
3. Clique na **Campaigns** aba
4. Clique na **Create Campaign** botão. Você será redirecionado para o formulário de criação de campanha.

![](/docs/_remote/6288b97cb788a168a929488f.webp)

5. Preencha os seguintes campos:

* **Campaign Name**: Escolha um nome para sua campanha. Esse nome será visível para os usuários na página de recompensas deles.
* **Campaign Network**: Selecione a rede em que seus wearables serão mintados. Observe que essa escolha é definitiva e não poderá ser alterada depois que a campanha for criada.
* **Max Gas Price**: Defina o preço máximo de gas que você está disposto a pagar por transação. Se o preço de gas de uma transação exceder esse limite, ela será adiada até que o preço diminua. A UI fornecerá uma estimativa do custo para mintar cada item com base nessa configuração.

6. Clique nos **Create Campaign** botão. Você será redirecionado para a página da campanha.

![](/docs/_remote/2c39dab8808bf032e1c51198.webp)

Depois de criar sua campanha, há mais algumas etapas a serem concluídas antes que você possa começar a entregar recompensas:

* **Adicione uma reserva de Tokens MATIC**: Certifique-se de ter tokens MATIC suficientes em sua conta para cobrir as taxas de transação.
* **Adicione itens ao estoque da campanha**: Preencha sua campanha com os itens que você planeja oferecer como recompensas.
* **Crie um dispenser**: Configure um dispenser que será responsável pela distribuição de recompensas aos usuários.
* **Ative sua campanha**: Por fim, ative sua campanha para colocá-la em funcionamento e começar a entregar recompensas.

### Adicione MATIC para pagar as taxas de transação

Para mintar itens, você precisará cobrir as taxas de transação adicionando uma reserva de tokens MATIC ao endereço da sua campanha. Você pode encontrar esse endereço listado abaixo do nome da campanha.

Para transferir MATIC da sua Wallet, primeiro certifique-se de estar conectado à mesma rede da campanha. Em seguida, copie o endereço da campanha e envie a quantidade desejada de MATIC para ele. Se você não tiver MATIC na sua Wallet, poderá seguir [este guia](https://github.com/decentraland/docs/tree/main/player/blockchain-integration/transactions-in-polygon.md#where-can-i-get-matic-to-pay-for-transaction-fees) para adquirir alguns.


> [!NOTE]
> **💡 Dica**: Para campanhas executadas na rede de teste **AMOY** você pode obter alguns MATIC em [esta faucet](https://faucet.polygon.technology/).


Para enviar MATIC a partir de uma exchange, basta sacar o token MATIC para a Polygon Network (geralmente a Amoy Network não está disponível em exchanges).

### Adicione itens ao estoque da campanha

Quando você cria uma nova campanha, ela começa com um inventário vazio. Para começar a entregar recompensas, você precisará adicionar itens à campanha:

![](/docs/_remote/ec2d216749ea170e1a5f766a.webp)

* **Conceda permissões de Minting**: Primeiro, autorize a campanha a mintar itens. Siga [este guia](/docs/creator/content-creator-pt/wearables-e-emotes/publicacao-de-colecoes/publishing-collections/#adding-minters-to-the-collection) e use o endereço da campanha (localizado abaixo do nome da campanha) como o endereço do minter. Após enviar, aguarde a confirmação da transação.
* **Adicione itens ao estoque da campanha**: Depois que as permissões de minting forem concedidas, você poderá começar a adicionar itens ao inventário da campanha.

  a) Clique no botão **Add Supply** para visualizar todas as coleções para as quais a campanha tem permissões de minting.

![](/docs/_remote/0e60e96b9eeeac0ec6fcde02.webp)

```
b) Selecione o item da coleção que deseja adicionar ao estoque e, em seguida, clique no botão **Add Supply**.
```

![](/docs/_remote/f1349ec843ec2ae7fa0e04a0.webp)

```
c) Selecione a quantidade de itens que deseja adicionar ao estoque da campanha e clique no botão **Add supply**.
```

![](/docs/_remote/1281e9ab96332e98d58a7e9a.webp)


> [!NOTE]
> Os campos Priority e Group são recursos avançados que serão abordados em uma seção dedicada.


### Crie um dispenser para entregar as recompensas

Dispensers são as ferramentas usadas para enviar ou reivindicar recompensas do seu estoque. Cada dispenser está associado a uma chave única, necessária para autenticação com a API HTTP. Embora a chave não seja exibida na UI, você pode copiá-la clicando no botão Copy.

Quando você cria uma nova campanha, um dispenser padrão com uma chave mestra é fornecido automaticamente apenas para fins de teste. Para configurar um novo dispenser, clique no **Add Dispenser** button.

![](/docs/_remote/80ef5f28ed3dbc20c04f5aaa.webp)

Selecione o grupo de itens de onde deseja retirar itens e clique em **Save** button.


> [!NOTE]
> Os dispensers têm algumas opções de configuração que serão abordadas em uma seção dedicada.


### Ative sua campanha

Por padrão, sua campanha está inativa. Isso impede que o serviço de recompensas atribua novas recompensas. Para ativar sua campanha, clique em **Activate campaign** button.


> [!WARNING]
> **📔 Nota**: Se uma campanha ativa for definida como inativa, todas as recompensas pendentes ainda serão enviadas. No entanto, novas reivindicações ou reivindicações que precisem ser tentadas novamente por algum motivo serão impedidas.


**Agora você está pronto para começar a mintar wearables/emotes usando a** [**API**](/docs/creator/content-creator-pt/recompensas/integrations/)

### Gerencie fundos

Depois que a campanha for concluída, se restarem fundos das taxas de transação, o proprietário poderá escolher entre recuperar os fundos ou reatribuí-los a outra campanha. Para recuperar fundos, basta clicar no botão Manage Funds na página de detalhes da campanha.

![](/docs/_remote/3ff911494b211a0d72e187c5.webp)


> [!WARNING]
> **📔 Nota**: Se todos os fundos forem removidos de uma campanha, quaisquer transações pendentes ou novas relacionadas a essa campanha serão bloqueadas até que mais fundos sejam atribuídos.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/recompensas/getting-started.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
