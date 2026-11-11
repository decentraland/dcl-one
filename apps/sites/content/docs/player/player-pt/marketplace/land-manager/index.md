> Gerencie LAND e tokens de Estate

# Gerenciador de LAND

O Land Manager permite-lhe gerir os seus ativos LAND e Estate.

Aceda ao Land Manager em <https://builder.decentraland.org/land>.

O Land Manager permite-lhe:

* **Nomear** os seus parcels e Estates e dar-lhes uma descrição pública.
* **Unir** parcels LAND num Estate.
* **Dissolver** um Estate em parcels LAND separados.
* **Transferir** os seus parcels e Estates para outro utilizador.
* **Conceder permissões** a outros utilizadores para editarem os parcels que possui.

## Gerir o seu LAND

Para ver os seus tokens LAND, clique **Gerir o seu LAND**. Aqui encontrará uma lista de todos os seus parcels e Estates, incluindo quaisquer parcels que tenha à venda.

Ao clicar num dos parcels ou Estates listados em **Land**, pode editar o nome, a descrição, definir um operador ou transferi-lo diretamente para outro endereço de wallet.

![](/docs/_remote/2442e30de2eeb3bfbb62d341.webp)

## Criar um Estate

Os Estates LAND tornam possível associar dois ou mais parcels de LAND diretamente adjacentes para facilitar a gestão das suas holdings de LAND maiores. Os Estates são especialmente úteis ao construir scenes maiores que abrangem mais do que um parcel.

Os parcels num Estate têm de ser diretamente adjacentes e não podem ser separados por uma estrada, praça ou qualquer outro parcel.

Para criar o seu primeiro Estate, precisa de possuir dois ou mais parcels LAND adjacentes.

1. Abra **O meu LAND** e selecione um dos parcels que gostaria de adicionar ao Estate.
2. Na página de detalhes do parcel, clique em **Criar Estate**.
3. Ser-lhe-á apresentada uma vista do Atlas centrada no parcel que selecionou, com os parcels adjacentes restantes que possui destacados. Selecione os diferentes parcels que pretende incluir no seu Estate. ![](/docs/_remote/4ef95117e0b2e23f246636d8.webp) ![](/docs/_remote/5d4f1b545b9d7baf0d140f45.webp)
4. Clique **Continuar**.
5. Introduza um nome e uma descrição para o seu Estate. Estes detalhes serão apresentados publicamente no Atlas, tal como o nome e a descrição de qualquer parcel individual.

   ![](/docs/_remote/75a307fdd4e3535bf406e003.webp)
6. Confirme esta transação no seu cliente Ethereum e aguarde a rede verificá-la.

Depois de criar o seu primeiro Estate, verá um novo separador chamado Estates. Nesta página, pode ver e gerir todos os seus Estates.

Quando cria um novo Estate, está efetivamente a transferir os seus parcels para um novo token. Estes Estates são representados por tokens ERC721 (como qualquer outro NFT). Já não verá os parcels individuais em *O meu LAND*, e não aparecerão em wallets MetaMask, Mist, Trezor ou Ledger, nem no Etherscan sob o seu endereço.

## Editar parcels ou Estates

Pode editar o nome e a descrição de qualquer parcel ou Estate que possua. Estes detalhes serão apresentados publicamente no Atlas.

Para editar um parcel ou Estate:

1. Navegue para a página de detalhes do parcel ou do Estate que gostaria de editar e clique em **Editar**.

   ![](/docs/_remote/3dc2640de68eb6f3a61f97b6.webp)
2. Clique **Enviar**.
3. Confirme esta transação no seu cliente Ethereum e aguarde a rede verificá-la.

## Conceder permissões

Pode conceder a outro utilizador permissões para editar o conteúdo num parcel ou Estate. Isto permite que esse utilizador faça deploy de código para o scene, sem ter a capacidade de vender o token.

O utilizador com permissão concedida também pode alterar o nome ou a descrição no Marketplace.

Para conceder permissões sobre o seu LAND:

1. Navegue para a página de detalhes do parcel ou Estate e clique nos três pontos e depois em **Definir operador**. ![](/docs/_remote/1ebd7dd1f5d8b72647811f3b.webp)
2. Preencha o formulário com as informações pretendidas.

   ![](/docs/_remote/8de80dd1532ffea8a620d5b2.webp)
3. Clique **Enviar**.
4. Confirme esta transação no seu cliente Ethereum e aguarde a rede verificá-la.

## Veja o seu histórico de atividade

Abra a página de notificações clicando no ícone do sino no topo do ecrã.

![](/docs/_remote/350d35176446e1c73654bfae.webp)

A página de notificações exibe uma lista de todas as transações recentes que realizou, juntamente com o respetivo estado.

Clique numa transação para ver mais detalhes sobre ela no Etherscan.

## Transferir LAND

Para transferir uma parcela de LAND ou um Estate para outro usuário:

1. Navegue para a página de detalhes do parcel ou do Estate que gostaria de transferir e clique em **Transferir**.

   ![](/docs/_remote/d4abb6744a7112af5514a3f1.webp)
2. Insira o endereço público da Wallet Ethereum do destinatário.


> [!WARNING]
> **📔 Observação**: Por favor, verifique este endereço com atenção, pois você não pode cancelar a operação. Embora o destinatário sempre possa transferir o LAND de volta para você, o proprietário original não pode reverter a ação.


![](/docs/_remote/186df165a7bc48578054cfb9.webp)

3. Clique **Enviar**.
4. Confirme esta transação no seu cliente Ethereum e aguarde a rede verificá-la.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/marketplace/land-manager.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
