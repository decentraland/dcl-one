> Aluguéis de LAND

# Aluguéis

## Glossário

**Proprietário de LAND:** Conta (endereço) que possui LAND, podendo ser um Parcel, um Estate, ou ambos.

**Inquilino:** Conta (endereço) que aluga LAND de um Proprietário de LAND. Esta também é a única Conta que pode alterar o Endereço que tem Permissões de Operador.

**Permissão de Operador:** O endereço com essa permissão é o único que pode implantar scenes nessa LAND.

**Transações:** Transações da blockchain Ethereum que custam gas.

## Introdução

O novo Sistema de Aluguel permite que Proprietários de LAND e Inquilinos **aluguem LAND de uma forma segura e sem necessidade de confiança** usando uma combinação de assinaturas armazenadas em um servidor administrado pela Decentraland Foundation (off-chain) e transações Ethereum (on-chain).

Por exemplo, um DJ poderia encontrar um terreno legal de LAND, alugá-lo e implantar um nightclub para tocar todos os sábados. Uma universidade poderia alugar um Estate e construir um campus para seus estudantes.

Abaixo você encontrará todas as etapas que precisa seguir para alugar uma LAND, e as transações envolvidas para ambas as partes.

## Para Proprietários de LAND

### Listar LAND para Aluguel

Como Proprietário de LAND, você pode listar sua LAND (Parcels ou Estates) para Aluguel no [Marketplace](https://market.decentraland.org/) > My Assets > LAND.

Para fazer isso on-chain, o Proprietário de LAND precisa aprovar o Rent Smart Contract para usar a LAND em seu nome. Então, cada anúncio também precisaria de uma assinatura do Proprietário.

![](/docs/_remote/1a34b8c7a8542b15716940f1.webp)

Você pode definir um preço de aluguel por dia em MANA e a quantidade de dias em que deseja permitir que as pessoas a aluguem. O preço por dia multiplicado pelo número de dias no período é o que o inquilino pagará **antecipadamente, e no total** por esse aluguel.

![](/docs/_remote/d0b5696229c03215afc73455.webp)

Depois de definir o Preço por Dia, você precisa selecionar o número de dias que os Usuários podem alugar sua LAND. Por exemplo, se você selecionar apenas 7 e 30 dias, o Inquilino só poderá escolher entre essas 2 opções. Caso a opção de 30 dias seja selecionada pelo Inquilino, essa será a duração do aluguel a partir do dia em que ele for confirmado.

![](/docs/_remote/f0859bad5d58c27a481d636a.webp)

Você também pode definir uma data de expiração para o anúncio. Isso significa que, se a LAND não tiver sido alugada até a data selecionada, o anúncio será removido do Marketplace. Além disso, o smart contract rejeitará a assinatura expirada para que ninguém possa alugá-la pelo preço e pela duração do anúncio selecionados anteriormente. Esta é uma medida de segurança para evitar que ela seja alugada por um preço ou duração indesejados.

![](/docs/_remote/f4c314e8b64de94a0f2045e5.webp)

Depois que o preço, o período de aluguel e a data de expiração do anúncio forem definidos, sua LAND aparecerá como disponível para aluguel no Marketplace.


> [!NOTE]
> 💡 Quando a LAND é alugada por um Inquilino, ela não pode ser vendida até ser recuperada. Lances de potenciais compradores também não podem ser recebidos.



> [!NOTE]
> 💡 O Poder de Voto é mantido pelo Proprietário de LAND, mesmo que ela esteja alugada.


### Editar ou Cancelar um Anúncio

Depois que a LAND for Listada para Aluguel no Marketplace, e antes que alguém a alugue, você pode editar as condições do Anúncio clicando no ícone de lápis nos detalhes da LAND. Você também pode remover o Anúncio do Marketplace e da blockchain.

![](/docs/_remote/89bcf7626763f867ddbc8c00.webp)


> [!NOTE]
> 💡 Editar e cancelar exigem uma transação, que custa gas. Veja a seção Transações abaixo para mais detalhes.


### Depois que o Aluguel termina

Depois que o Aluguel termina, você pode **Recuperar sua LAND ou Listá-la para Aluguel Novamente**.

**As Permissões de Operador não são transferidas automaticamente de volta para o Proprietário de LAND**. Para recuperá-las, o Proprietário de LAND precisa recuperar a LAND enviando essa transação e pagando a taxa de gas. Confirmar a transação removerá as Permissões de Operador do Inquilino e as devolverá ao Proprietário de LAND.

![](/docs/_remote/f43022686a5bd15b6c02ac8a.webp)

A outra possibilidade é Listar a LAND para Aluguel Novamente, em vez de recuperá-la. Isso não exigirá pagar por outra transação, mas **as Permissões de Operador serão mantidas pelo Inquilino anterior até que um novo Inquilino confirme um novo Aluguel.**

O Proprietário de LAND pode editar o preço, o período de aluguel e a data de expiração do anúncio para o novo anúncio.

![](/docs/_remote/6f6b27950ff42602c5b34fda.webp)

Ambas as ações podem ser feitas na página de detalhes da LAND no Marketplace.

![](/docs/_remote/2b6877fde55556d4100fd7e0.webp)

### Status do Aluguel

Você pode verificar o Status de qualquer LAND alugada em My Assets > Store > On Rent. Os status possíveis são:

* Listada para Aluguel - O anúncio foi confirmado e está disponível para os usuários alugarem no Marketplace
* Período de Aluguel Encerrado - Nesta etapa, a LAND está disponível para ser Recuperada ou Listada Novamente para Aluguel pelo Proprietário de LAND
* Alugada até *"date"* - A LAND já está alugada e o Inquilino tem Permissões de Operador até que ela seja recuperada ou alugada por outro usuário

![](/docs/_remote/486197d9f7fa915ceb7f0183.webp)

## Para Inquilinos

### Alugar LAND

Todos os usuários podem encontrar LAND listada para aluguel no Marketplace, na seção LAND.

![](/docs/_remote/95cd665129b4cbe803dc76df.webp)

Há LANDs disponíveis para Venda ou Aluguel. Caso ambas as opções estejam disponíveis, você pode ver as condições disponíveis para cada uma clicando no toggle Sale/Rent.

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

Depois de encontrar a LAND que deseja alugar, você precisa selecionar o Período de Aluguel, ou seja, os dias em que você terá a LAND. Depois de selecionar o Período de Aluguel, você verá o preço total a ser pago pelo Aluguel.

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

Você precisará aprovar o Rent Smart Contract para retirar o MANA da sua conta antes de prosseguir.

Antes de confirmar o Aluguel, você pode decidir quem gerenciará a LAND (Permissão de Operador). Pode ser você mesmo ou qualquer outro endereço que escolher.

![](/docs/_remote/2d83c8c5fc075dc07df8532c.webp)

A Permissão de Operador pode ser alterada depois pelo Inquilino (o endereço que alugou a LAND inicialmente) a partir do [Builder](https://builder.decentraland.org/).

![](/docs/_remote/506dbf3015f1fd2ea5698b48.webp)

Depois de selecionar todos os detalhes e aprovar o Rent Smart Contract para lidar com o seu MANA, você pode confirmar o Aluguel enviando uma transação.

E está tudo pronto! você pode começar a trabalhar na sua LAND e implantar uma scene usando o Builder ou o SDK.

![](/docs/_remote/76317d506da755beb44eeaf7.webp)

Observação: depois que o Aluguel termina, o Inquilino ainda terá Permissões de Operador até que o Proprietário de LAND a recupere, ou outra pessoa a alugue. **Certifique-se de salvar seu conteúdo antes do fim do aluguel, caso contrário ele poderá ser perdido.**


> [!NOTE]
> 💡 Alugar LAND não transfere o Poder de Voto para o Inquilino. O Poder de Voto é mantido pelo Proprietário de LAND conforme definido pela DAO nesta [Proposal](https://governance.decentraland.org/proposal/?id=c98bd010-74b1-11ed-a9bf-f772a12a0556)


## Transações

Por motivos de **segurança e descentralização**, o sistema de Aluguel depende da blockchain Ethereum como fonte da verdade.

Mas nem toda ação envolvida exige uma entrada na blockchain. Se esse fosse o caso, seria caro demais para ambas as partes.

As transações na blockchain são mínimas para fornecer um **sistema robusto e sem necessidade de confiança para o aluguel de LAND, mantendo-o acessível.** Estas são todas as transações a considerar:

### Para Proprietários de LAND

#### Listar para Aluguel

Antes de listar o primeiro Parcel ou Estate para aluguel, os Proprietários de LAND precisam permitir que o Rents Smart Contract opere a LAND em seu nome. Isso precisa ser feito apenas uma vez para Parcels e apenas uma vez para Estates.

![](/docs/_remote/24e324959f412a5ef38535ed.webp)

#### Recuperar a LAND ou Listar para Aluguel Novamente

Depois que o período de aluguel termina, **As Permissões de Operador não são transferidas automaticamente de volta para o Proprietário de LAND**. Para recuperá-las, o Proprietário de LAND precisa recuperar a LAND enviando essa transação e pagando a taxa de gas.

![](/docs/_remote/9b7b0037cff4903f1d9e7022.webp)

Outra possibilidade é Listar a LAND para Aluguel Novamente, em vez de recuperá-la. Isso não exigirá pagar por outra transação, mas as Permissões de Operador serão mantidas pelo Inquilino anterior até que um novo Inquilino confirme um novo Aluguel.

#### Editar Anúncio

Se o Preço, o Período de Aluguel ou a Data de Expiração forem alterados, uma transação precisa ser enviada pelo Proprietário de LAND para se proteger de alguém usar a assinatura do anúncio anterior diretamente no Smart Contract (não pela UI do Marketplace) e obtê-la por um preço menor do que o desejado ou por uma duração indesejada.

![](/docs/_remote/985e0faa31bf88ac207e0846.webp)

### Para Inquilinos

#### Permitir que o Rent Contract opere o seu MANA

Quer seja um Parcel ou um Estate, todo usuário que deseja alugar LAND precisa enviar uma transação para permitir que o Rent Smart Contract opere MANA em seu nome. Isso é necessário porque o Smart Contract precisa retirar o MANA e transferi-lo para o Proprietário de LAND quando o aluguel é ativado. Isso é feito apenas uma vez para toda a LAND que vier a ser alugada a partir desse momento.

#### Alugar LAND

Depois de aprovar o Rent Smart Contract para operar o seu MANA, você estará pronto para confirmar seu primeiro Aluguel. Depois de encontrar a LAND que deseja, escolha o período de aluguel e confirme a transação de Aluguel; as Permissões de Operador são transferidas para o endereço selecionado.

Se você quiser alugar outro Parcel ou Estate, só precisa enviar uma transação para confirmá-lo; não há necessidade de aprovar o Smart Contract para operar o seu MANA novamente.

![](/docs/_remote/34fb306b490b37363a4d6562.webp)

#### Alterar Operador

No momento de alugar a LAND, o usuário pode escolher qual endereço terá Permissões de Operador para aquela LAND. Se esse endereço quiser ser alterado, uma transação precisa ser enviada.

![](/docs/_remote/e38a6d9159fa451ba7b2570b.webp)

## Smart Contract Wallets

A funcionalidade de Rentals depende fortemente de assinaturas off-chain. As ações off-chain permitem que os Proprietários de LAND listem LANDs para aluguel sem pagar o custo da transação.

Ao assinar um anúncio, o Rent Smart Contract pode verificar se o anúncio foi criado pelo signatário.

Assinar tem a particularidade de exigir uma chave privada. Todas as EOA (Externally Owned Accounts) têm uma, e podem assinar anúncios com ela. O Rentals Smart Contract então verificará a assinatura gerada pela EOA ao executar um aluguel.

Smart Contract Wallets, que são Smart Contracts, não possuem chave privada, portanto não conseguem assinar mensagens. Em vez disso, uma EOA autorizada pela Smart Contract Wallet precisa assinar.

Para suportar essas assinaturas, o Rent Smart Contract verifica com a Smart Contract Wallet se a assinatura é válida seguindo o [EIP-1271](https://eips.ethereum.org/EIPS/eip-1271) padrão. Se a assinatura for válida, o aluguel pode ser executado.

A Smart Contract Wallet não precisa apenas ter o método padrão de verificação de assinatura definido no EIP-1271, mas também o método receptor de tokens definido no [padrão EIP-721](https://eips.ethereum.org/EIPS/eip-721). Isso é necessário ao recuperar a LAND, porque o Rent Smart Contract fará um `safeTransferFrom` para devolver o NFT à Smart Contract Wallet, e se ela não tiver implementado a `onERC721Received` função apropriada, não será possível recuperar a LAND.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/marketplace/rentals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
