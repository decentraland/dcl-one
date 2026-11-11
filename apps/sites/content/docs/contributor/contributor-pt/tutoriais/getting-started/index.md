# Introdução

Já ficou curioso sobre como funcionam os nós Catalyst do Decentraland? Já quis executar seu próprio nó e se sentiu sobrecarregado sem saber por onde começar? Você está criando conteúdo para o Decentraland e está achando difícil fazer todo o ciclo de desenvolvimento e testes em servidores de produção?

Se você se identifica (ou já passou por isso), não tema mais. Este tutorial vai ajudá-lo no processo de planejamento, a descobrir o que você precisa decidir e depois guiá-lo passo a passo para iniciar o servidor. Além disso, caso algo não saia conforme o planejado, há uma seção de solução de problemas no final com alguns dos problemas mais comuns e como corrigi-los.

Pronto? Mãos à obra.

### Requisitos de hardware

Os servidores da Fundação Decentraland estão implantados na Amazon AWS em `t2.xlarge` instâncias. Portanto, embora esses servidores sejam destinados ao uso público, o seu próprio pode ficar bem com especificações bem menores, dependendo de qual será o uso pretendido.

Uma instância AWS EC2 `t2.xlarge` tem o seguinte hardware:

* 4 vCPUs.
* 16 GB de RAM

E para armazenamento de arquivos + armazenamento do banco de dados, um SSD / HDD com capacidade de 2 TB deve ser mais do que suficiente. No momento em que isto foi escrito, os servidores da fundação estão usando um pouco mais de 1 TB para todo o armazenamento.

### Pré-requisitos de software

Os seguintes são necessários para executar catalyst-owner:

* Sistema operacional Linux / MacOS
* Docker / docker-compose
* Git
* cluster LiveKit

### Algumas opções a considerar antes de começarmos

* Você quer que seu nó fique exposto publicamente na Internet? Nesse caso, você precisará de uma instância exposta à Internet e de uma URL pública para se conectar a ela.
* O nó será usado para o desenvolvimento de scenes, wearables etc.? Nesse caso, as especificações de hardware podem ser muito menores, já que provavelmente serão uma ou duas pessoas acessando-o, e não centenas de usuários.
* Você quer sincronizar todos os tipos de entidade? Se estiver usando isso para desenvolver scenes, provavelmente você não precisará sincronizar todos os tipos de conteúdo. Você pode definir a variável de ambiente

### LiveKit

Um cluster LiveKit externo é necessário para que o serviço Comms do Catalyst orquestre as comunicações entre os jogadores. Para isso, há duas opções: usar uma conta LiveKit Cloud ou executar seu próprio cluster LiveKit.

A recomendação é configurar [uma conta LiveKit Cloud](https://cloud.livekit.io/) que vem com um plano gratuito capaz de gerenciar 100 usuários, ou pagar pelo serviço de acordo com o tráfego. Essa abordagem é mais fácil, pois não exige provisionar infraestrutura extra e o serviço pode gerenciar a escala. Caso contrário, será necessário provisionar um cluster LiveKit. A LiveKit faz um ótimo trabalho com sua documentação:

* [Implantação](https://docs.livekit.io/oss/deployment/)
* [Configuração distribuída](https://docs.livekit.io/oss/deployment/distributed/)

*Caso queira se aprofundar nos detalhes técnicos necessários para dar suporte a comunicações baseadas em transporte do LiveKit, confira o* [*ADR-70 New Communications Architecture*](https://adr.decentraland.org/adr/ADR-70)*.*

### Guia passo a passo

A primeira coisa a fazer é clonar o [catalyst-owner](https://github.com/decentraland/catalyst-owner) repositório do GitHub

```bash
> git clone https://github.com/decentraland/catalyst-owner.git
Clonando em 'catalyst-owner'...
remote: Enumerando objetos: 2392, concluído.
remote: Contando objetos: 100% (602/602), concluído.
remote: Compactando objetos: 100% (168/168), concluído.
remote: Total 2392 (delta 506), reutilizados 481 (delta 430), pack-reused 1790
Recebendo objetos: 100% (2392/2392), 926.05 KiB | 1.09 MiB/s, concluído.
Resolvendo deltas: 100% (1257/1257), concluído.
> cd catalyst-owner
```

Depois disso, é hora de configurar as variáveis de ambiente para o nó e fazer quaisquer alterações necessárias.

```bash
# Na pasta /catalyst-owner
> cp .env.example .env
> cp .env-advanced.example .env-advanced
```

Agora, usando seu editor de texto favorito, faça as alterações necessárias nesses arquivos. Por exemplo, configurar a `EMAIL` variável de ambiente com um e-mail válido é necessário para que você possa receber atualizações sobre a expiração do certificado do Certbot.

Além disso, definir o valor específico para `CATALYST_URL` pode ser necessário, especialmente se o seu servidor for exposto publicamente na Internet. Para uso na sua máquina local, o padrão `http://localhost` serve.

**Serviço de Comms**

Assim que o cluster LiveKit estiver disponível, você precisará definir as variáveis específicas do LiveKit para que o serviço Comms funcione, por exemplo:

```
LIVEKIT_HOST=wss://livekit-1.mydomain.org
LIVEKIT_API_KEY=API-JjHuvM
LIVEKIT_API_SECRET=J7YSHmNzkNCEfT2
ROOM_PREFIX=my-prefix
```

O `ROOM_PREFIX` variável é opcional e pode ser configurada para identificar as salas LiveKit criadas pelo Catalyst. Isso é útil se mais de um Catalyst estiver usando o mesmo cluster LiveKit.

**Servidor de conteúdo**

Há uma variável chamada `CONTENT_SERVER_STORAGE` que define a pasta local que o content server usará para armazenamento. Isso tem como padrão `CONTENT_SERVER_STORAGE=./storage`. Você pode alterar esse valor para armazenar os arquivos em outro lugar, ou pelo menos garantir que a pasta referenciada exista. Talvez você precise criá-la assim:

```bash
# Na pasta /catalyst-owner ou onde quer que CONTENT_SERVER_STORAGE aponte
> mkdir storage
```

Há outra variável interessante `SYNC_IGNORED_ENTITY_TYPES` que permite ignorar certos tipos de entidade durante o processo de sincronização. Se você for usar o servidor Catalyst para desenvolvimento, talvez não precise que todos os tipos de conteúdo sejam sincronizados. Você pode definir a variável de ambiente `SYNC_IGNORED_ENTITY_TYPES="profile,store"` para que apenas scenes e wearables sejam obtidos dos servidores da DAO. Isso economizará muito tempo, largura de banda e armazenamento em disco no seu servidor.

> 💡 Para uma lista mais abrangente de todas as variáveis de ambiente suportadas, dê uma olhada na [Variáveis de ambiente](#environment-variables) seção abaixo.

#### Iniciar o nó Catalyst

Depois que todas as variáveis de ambiente tiverem sido configuradas, é hora de iniciar o nó. Isso deve ser tão fácil quanto executar o `init.sh` script na pasta raiz.

```bash
# Na pasta /catalyst-owner

> ./init.sh
## Carregando variáveis de ambiente... [ OK ]
[ OK ]
## Verificando se o e-mail está configurado... [ OK ]
## Verificando se o armazenamento está configurado... [ OK ]
## Verificando se a URL do catalyst está configurada... [ OK ]
 AVISO: Você não está executando a imagem mais recente dos nós Content e Lambdas do Catalyst.
 AVISO: Você não está executando a imagem mais recente do nó Archipelago do Catalyst.
 AVISO: Você não está executando a imagem mais recente do nó Explorer BFF do Catalyst.
 - DOCKER_TAG:               45a5154f11b53d55aadfdf7f958e2fce6a964824
 - LIGHTHOUSE_DOCKER_TAG:    latest
 - CATALYST_URL:             http://localhost
 - CONTENT_SERVER_STORAGE:   ./storage
 - EMAIL:                    a@a.com
 - ETH_NETWORK:              mainnet
 - REGENERATE:               0

Iniciando em 5 segundos...
45a5154f11b53d55aadfdf7f958e2fce6a964824: Baixando de decentraland/catalyst-content
Digest: sha256:d6c2981c57cb9367fdb3228d15b07e4d26e62c902944115937de24ecd3ab6aac
Status: A imagem está atualizada para decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824
docker.io/decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824
45a5154f11b53d55aadfdf7f958e2fce6a964824: Baixando de decentraland/catalyst-lambdas
Digest: sha256:5605812dd29316afc32e561d2a2ce1311f164f07ef3cd99a1cd60727518636e5
Status: A imagem está atualizada para decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824
docker.io/decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824
latest: Baixando de decentraland/catalyst-lighthouse
Digest: sha256:fedc10b714823909f0a1c17955eed2f22a02e4f784090848d3253ef734e410d4
Status: A imagem está atualizada para decentraland/catalyst-lighthouse:latest
docker.io/decentraland/catalyst-lighthouse:latest
latest: Baixando de decentraland/archipelago-service
Digest: sha256:e2c1d6fa96a5cfbbf6fb37e3840c0724fb5abeef8c366025a28fab3ad33d6480
Status: A imagem está atualizada para quay.io/decentraland/archipelago-service:latest
quay.io/decentraland/archipelago-service:latest
latest: Baixando de decentraland/explorer-bff
Digest: sha256:fa9e305c44972a8613b01f4cd573d6fb84b0fb03ce953a506fbb06053202913c
Status: A imagem está atualizada para quay.io/decentraland/explorer-bff:latest
quay.io/decentraland/explorer-bff:latest
Parando nginx ... concluído
## Usando HTTP porque CATALYST_URL está definido como http://localhost
## Substituindo o valor $katalyst_host no arquivo do servidor nginx... [ OK ]
## Reiniciando contêineres...
Criando rede "catalyst-owner_default" com o driver padrão
Criando catalyst-owner_comms-server_1 ... concluído
Criando node-exporter                   ... concluído
Criando catalyst-owner_lambdas_1      ... concluído
Criando postgres                      ... concluído
Criando catalyst-owner_certbot_1        ... concluído
Criando nats                            ... concluído
Criando postgres-exporter               ... concluído
Criando catalyst-owner_content-server_1 ... concluído
Criando catalyst-owner_archipelago_1    ... concluído
Criando catalyst-owner_explorer-bff_1   ... concluído
Criando nats-exporter                   ... concluído
Criando cadvisor                        ... concluído
Criando nginx                           ... concluído
## O servidor Catalyst está no ar e em execução em http://localhost
```

Se tudo correu bem, agora temos um nó completo do Decentraland em execução. Você pode ir ao navegador e digitar a URL. Se estiver usando o padrão, ela deve ser [`http://localhost`](http://localhost/).

Podemos verificar os logs do content server com este comando do Docker:

```bash
> docker logs catalyst-owner_content-server_1
```

Embora o servidor esteja em execução agora, ele ainda não está 100% pronto para entrar em operação. Ele precisa sincronizar o conteúdo dos outros [servidores da DAO](https://decentraland.github.io/catalyst-monitor) para que possa oferecer a mesma experiência aos usuários conectados a ele. A sincronização pode levar muito tempo. Em uma boa conexão de internet, 6 horas é bastante comum. Então... é hora de tomar um café, tirar uma soneca, descansar bem à noite e voltar amanhã.

Uma maneira de saber se a sincronização foi concluída consiste em verificar os resultados do endpoint de status do conteúdo: <http://localhost/content/status> (use sua URL aqui).

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153514917,
    "synchronizationState": "Inicializando"
  },
  "snapshot": {
    "entities": {
      "profile": 1271331,
      "scene": 23477,
      "wearable": 17351,
      "store": 890
    },
    "lastUpdatedTime": 1658153414530
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

Enquanto `synchronizationState` estiver em `Inicializando`, o nó suspenderá temporariamente a aceitação de novas implantações. Essa medida garante que nenhuma nova entidade seja implantada até que o nó esteja atualizado dentro da rede DAO. Assim que o status mudar para `Sincronizando`, isso indica que o nó já acompanhou o restante e agora recebe continuamente as atualizações mais recentes. Este é o estado saudável em que o nó está totalmente funcional e aceitando novas implantações.

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153872301,
    "synchronizationState": "Sincronizando"
  },
  "snapshot": {
    "entities": {
      "profile": 1271317,
      "scene": 23477,
      "store": 890,
      "wearable": 17350
    },
    "lastUpdatedTime": 1658152050030
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

Se você é mais do tipo CLI, pode usar grep nos logs da imagem Docker do content server para esta mensagem: `Iniciando a sincronização de entidades a partir das mudanças de ponteiros dos servidores`. Assim que você vir esse texto, o content server estará totalmente sincronizado e pronto para uso completo.

### Variáveis de ambiente

A seguir está uma lista abrangente de todas as variáveis de ambiente do content server com seus valores padrão, conforme registrados nos logs do content server durante a inicialização:

```jsx
STORAGE_ROOT_FOLDER: "/app/storage/content_server/"
DENYLIST_FILE_NAME: "denylist.txt"
DENYLIST_URLS: "https://config.decentraland.org/denylist"
SYNC_IGNORED_ENTITY_TYPES: ""
FOLDER_MIGRATION_MAX_CONCURRENCY: 1000
SERVER_PORT: 6969
LOG_REQUESTS: false
UPDATE_FROM_DAO_INTERVAL: 1800000
SYNC_WITH_SERVERS_INTERVAL: 45000
CHECK_SYNC_RANGE: 1200000
DECENTRALAND_ADDRESS: "0x1337e0507eb4ab47e08a179573ed4533d9e22a7b"
DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL: 60
DEPLOYMENTS_DEFAULT_RATE_LIMIT_MAX: 300
ETH_NETWORK: "mainnet"
LOG_LEVEL: "debug"
FETCH_REQUEST_TIMEOUT: "2m"
USE_COMPRESSION_MIDDLEWARE: false
BOOTSTRAP_FROM_SCRATCH: false
REQUEST_TTL_BACKWARDS: 1200000
LAND_MANAGER_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/land-manager"
COLLECTIONS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-ethereum-mainnet"
COLLECTIONS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-matic-mainnet"
THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/tpr-matic-mainnet"
BLOCKS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-ethereum-mainnet"
BLOCKS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-matic-mainnet"
PSQL_DATABASE: "content"
PSQL_HOST: "postgres"
PSQL_SCHEMA: "public"
PSQL_PORT: "5432"
GARBAGE_COLLECTION: true
GARBAGE_COLLECTION_INTERVAL: 21600000
PG_IDLE_TIMEOUT: 30000
PG_QUERY_TIMEOUT: 60000
PG_STREAM_QUERY_TIMEOUT: 600000
SNAPSHOT_FREQUENCY_IN_MILLISECONDS: 21600000
CUSTOM_DAO: undefined
DISABLE_SYNCHRONIZATION: false
SYNC_STREAM_TIMEOUT: "10m"
CONTENT_SERVER_ADDRESS: "http://localhost/content/"
REPOSITORY_QUEUE_MAX_CONCURRENCY: 50
REPOSITORY_QUEUE_MAX_QUEUED: 300
REPOSITORY_QUEUE_TIMEOUT: "1m"
ENTITIES_CACHE_SIZE: 150000
DEPLOYMENT_RATE_LIMIT_MAX: {}
DEPLOYMENT_RATE_LIMIT_TTL: {}
VALIDATE_API: false
RETRY_FAILED_DEPLOYMENTS_DELAY_TIME: 900000
```

### Usando seu nó em produção

Se você quer executar seu próprio servidor e ajudar a escalar a rede, antes de tudo isso é incrível; a comunidade e a fundação realmente agradecem por você fazer isso.

Nesse caso, você terá que passar pelo processo de solicitar aprovação da DAO para entrar na rede visitando [este link](https://governance.decentraland.org/submit/catalyst/). Você também pode pedir uma subvenção em MANA para cobrir as despesas de infraestrutura e gerenciamento.

É importante que as especificações do seu hardware estejam mais alinhadas com o que foi sugerido acima nos requisitos de hardware, pois o servidor será usado por qualquer membro da comunidade para acessar o Decentraland.

### Especificações da API

Para mais detalhes sobre as APIs dos servidores de conteúdo, lambdas e comms, você pode consultar as [especificações da API](https://decentraland.github.io/catalyst-api-specs).

### Solução de problemas/FAQ

Algumas coisas podem dar errado ao iniciar seu servidor. Aqui estão alguns dos problemas mais comuns encontrados ao longo do tempo e como corrigi-los.

#### A porta 5432 está sendo usada pelo postgres local

Se você executar o nó em uma máquina usada para desenvolvimento, ou se ela já estiver hospedando outros serviços, é provável que já exista um servidor de banco de dados postgres em execução nessa máquina. Portanto, a porta 5432 já estará ocupada.

A maneira mais fácil de corrigir isso é parar o postgres que já está em execução localmente e então reiniciar o contêiner Docker do postgres usando `docker start postgres`.

Se você tiver conhecimento de Docker Compose, pode começar a mexer em `docker-compose.yml` e nas variáveis de ambiente para tentar fazê-lo funcionar em uma porta diferente. Para manter as coisas simples aqui, evitaremos seguir por esse caminho neste tutorial.

#### A porta 80 está sendo usada pelo nginx local

A mesma coisa pode acontecer com a porta 80 (e até mesmo com a 443). Se você estiver executando nginx na máquina local, é provável que as portas 80/443 já estejam vinculadas a esse serviço, então o contêiner Docker do Catalyst não conseguirá usá-las.

Novamente, a abordagem mais simples seria parar o serviço local do nginx e depois reiniciar o contêiner Docker usando `docker start nginx`. Ou, alternativamente, você precisa usar `docker-compose.yml` e `.env` para fazê-lo funcionar em uma porta diferente.

### Documentação adicional para referência

Se você quiser entender melhor o que um servidor Catalyst faz, quais serviços ele inclui etc., você pode conferir o repositório de arquitetura <https://github.com/decentraland/architecture>, que explica as partes em detalhe.

Se você gostaria de contribuir com o código dos servidores Catalyst, não deixe de conferir o [guia de contribuição](https://github.com/decentraland/catalyst/blob/main/docs/CONTRIBUTING.md) para aprender sobre nosso processo de desenvolvimento, como propor correções de bugs e melhorias, e como construir e testar suas alterações.

E, por fim, se você precisar contatar a equipe, pode fazê-lo via [Discord](https://discord.com/channels/417796904760639509/948230185457696820) ou enviando [issues no GitHub](https://github.com/decentraland/catalyst/issues).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/tutoriais/getting-started.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
