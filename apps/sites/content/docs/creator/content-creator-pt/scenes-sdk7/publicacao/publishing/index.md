> Como publicar meu projeto?

# Publicação

## Antes de começar

Certifique-se do seguinte:

* A sua scene cumpre todas as [limitações da scene](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/). A maioria destas é validada sempre que executa uma preview da sua scene.
* Se quiser que a sua scene funcione bem para players em mobile, faça primeiro uma preview e teste-a num dispositivo real — veja [Building for Mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) e [Preview no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/). Depois de publicada, a sua scene pode ser acedida a partir da app móvel da Decentraland da mesma forma que no desktop.
* Tem uma [Metamask](https://metamask.io/) conta, com os seus parcels de LAND atribuídos a ela.
* Possui a quantidade necessária de parcels de LAND adjacentes ou um NAME da Decentraland. Caso contrário, pode comprar LAND no [Mercado](https://market.decentraland.org) ou um NAME no [Marketplace](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> \*\*📔 Nota\*\*: scenes com múltiplos parcels só podem ser publicadas em parcels adjacentes.



> [!CAUTION]
> **❗Aviso**: Ao planear eventos ao vivo, não faça alterações de última hora à scene mesmo antes do evento. Publique a versão final pelo menos uma hora antes de o evento começar.
> 
> Após cada publicação, um processo interno otimiza todos os modelos 3D antes de poderem ser renderizados. Isto normalmente demora apenas alguns segundos, mas pode demorar mais em scenes muito grandes ou quando os servers estão ocupados. Até terminar, é servida aos players a versão anterior da sua scene.


## Verificar dados da scene

Ao fazer deploy, a CLI lê informações do *scene.json* file para determinar onde fazer deploy da sua scene.

Abra o *scene.json* file da sua scene e preencha os seguintes dados:

* **título**: O título é apresentado na UI por baixo do mini-map, sempre que os players entram na sua scene. Também aparece no pop-up de teleport.
* **descrição**: Uma descrição do que os players encontrarão na sua scene. Isto é apresentado no pop-up de teleport.
* **navmapThumbnail**: Uma imagem que representa a sua scene. Isto é apresentado no pop-up de teleport. A imagem deve ser uma *.png* ou *.jpg* imagem com proporção 16:9, com um tamanho recomendado de *1920x1080* píxeis. Mantenha tudo o que for essencial dentro do quadrado central *1080x1080* quadrado, uma vez que algumas partes da plataforma apresentam um recorte quadrado. Veja [miniatura da scene](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#scene-thumbnail).
* **Parcelas**: As coordenadas dos parcels que serão ocupados pela sua scene
* **Base**: As coordenadas do parcel que será considerada a coordenada \[0,0] da scene. Se a sua scene tiver vários parcels, deve ser o parcel inferior esquerdo (sudoeste).
* **spawnPoints**: Um conjunto de coordenadas dentro da scene (relativas ao parcel base da scene) onde os players aparecem. Por predefinição, os players aparecem na *0,0,0* localização da scene (canto inferior esquerdo). Use isto para iniciar os players num local específico ou definir uma região para impedir que os players se sobreponham entre si quando aparecem pela primeira vez.
* **tags**: Um conjunto de tags que descrevem a sua scene. Isto ajuda os players a explorar a Decentraland, facilitando encontrar conteúdo do seu interesse. [Veja a lista de categorias disponíveis](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#categories).
* **classificação**: Isto é usado para classificar o conteúdo da sua scene. A Decentraland é uma plataforma para maiores de 18, por isso o valor a definir é `A` para Adults. Veja [Classificação etária](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#age-rating).


> [!WARNING]
> **📔 Nota**: Veja [metadados da scene](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/) para mais detalhes sobre como definir estes parâmetros.


## Para publicar a scene

### Usando o Scene Editor no Creator Hub

O Scene Editor no Creator Hub oferece uma forma fácil de publicar as suas scenes. Certifique-se de que tem [instalado o Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/).

1. Abra o seu projeto da scene.
2. Clique no **Publicar** botão no canto superior direito.
3. Uma janela pedirá para confirmar se deseja publicar para um **WORLD** ou para **LAND**.
   * Selecione **PUBLISH TO WORLD** para disponibilizar a sua scene num dos seus [WORLDs](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds). Em seguida, selecione para qual dos seus NAMEs ou domínios ENS pretende publicar.
   * Selecione **PUBLISH TO LAND** se for proprietário de land ou se lhe tiverem sido concedidas permissões de deploy por um proprietário. Depois selecione os parcels onde pretende fazer deploy no mapa. Os parcels onde lhe é permitido fazer deploy são apresentados a rosa.
   * Selecione **Servidores alternativos** para publicar num [servidor personalizado](#custom-servers).

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

4. Isto abre um novo separador no seu browser, mostrando detalhes sobre o upload. Assine o deploy com a sua wallet. Isto é apenas uma assinatura para provar a propriedade, é gratuito e não envolve qualquer transação na blockchain.
   * Para LAND numa conta Metamask no browser, confirme o deploy. Depois assine a mensagem na extensão Metamask do browser.
   * Para LAND ligado a uma wallet que pode usar via Wallet Connect, clique em **Connect wallet**, depois leia o código QR com o seu dispositivo móvel e siga os passos no Wallet Connect.

### Via a CLI

1. Inicie sessão na sua conta Metamask com o mesmo endereço público associado aos seus parcels na Decentraland.
2. Execute `npm run deploy` a partir da pasta da scene.


> [!NOTE]
> \*\*💡 Dica\*\*: Se houver ficheiros na pasta do seu projeto que não quer fazer deploy, liste-os no ficheiro \_.dclignore\_ antes de fazer deploy.


3. Será aberto um separador no browser, mostrando para que parcels está a fazer deploy. Clique em **Sign and Deploy**.
4. O Metamask abre, notificando que a sua assinatura foi solicitada. Clique em **Sign** para confirmar esta ação.


> [!NOTE]
> **💡 Dica**: Se estiver a implementar um fluxo de integração contínua, em que as alterações à sua scene são publicadas automaticamente, então pode definir a variável de ambiente `export DCL_PRIVATE_KEY` para a chave privada de uma conta que tenha permissões de deploy.



> [!NOTE]
> **💡 Dica**: `npm run deploy` executa um `npm run build`, que verifica a scene quanto a erros de tipo de forma mais rigorosa do que executar `npm run start`. Se estes erros não puderem ser evitados (por exemplo, ocorrem numa biblioteca externa) e não afetarem a scene, pode usar `npm run deploy -- --skip-build` para ignorar a `npm run build` etapa e publicar a scene tal como está.


## Publicar em Worlds

Para publicar a sua scene num Decentraland World, precisa de possuir um NAME da Decentraland ou um domínio ENS. Veja [Opções de publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) para informações sobre como obter um.

### Configurar scene.json

Tem de especificar sob que **nome** o seu deploy deve ser feito. Adicione a seguinte secção no seu `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth"
	}
}
```

O **nome** especificado no `scene.json` pode ser um NAME da Decentraland ou um domínio ENS. Tem de ser propriedade da wallet que assina o deploy, ou de qualquer wallet a quem tenha sido concedida permissão através de Access Control Lists (ACL).

Tenha o seguinte em mente:

* A wallet que assina o deploy deve ser proprietária do NAME especificado no `scene.json` file
* A scene não tem limitações de parcels (desde janeiro de 2023)
* Todos os Worlds são automaticamente listados na página Places, a menos que opte por não participar, como detalhado abaixo

### metadata do World

Um World tem o seu próprio nome, descrição e miniatura, separados dos metadados das scenes publicadas nele. Quando publica para um World vazio, ou para um World que contém uma única scene, os metadados do World são atualizados para corresponder aos metadados da scene em cada publicação. Se o World contiver mais de uma scene, os seus metadados são independentes: publicar uma scene não os altera, e só podem ser editados na **Gerir** secção do Creator Hub. Veja [World metadata vs scene metadata](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#world-metadata-vs-scene-metadata).

### orçamento de armazenamento dos Worlds

As scenes publicadas em Worlds contam para um orçamento de armazenamento partilhado entre todos os Worlds propriedade da sua wallet. O orçamento aumenta com os seus ativos na Decentraland:

* Cada NAME da Decentraland que possuir concede 100 MB.
* Cada parcel de LAND que possuir concede mais 100 MB.
* A cada 2.000 MANA que tiver na sua wallet, são concedidos mais 100 MB.

É livre de distribuir este orçamento entre os seus Worlds como quiser. Por exemplo, com um orçamento de 500 MB, poderia publicar uma scene de 300 MB e uma scene de 200 MB, ou cinco scenes de 100 MB. Os Worlds publicados para domínios ENS não consomem este orçamento; têm antes um limite fixo de 36 MB por World.

Pode verificar quanto do seu orçamento está usado e quanto resta em dois locais:

* O **Gerir** secção do Creator Hub. Clique em **Ver detalhes** para ver o detalhe de como os seus ativos MANA, LAND e NAME se somam.
* O **Worlds** separador do [Builder](https://decentraland.org/builder/worlds).

Se exceder o seu orçamento (por exemplo, depois de vender ou transferir ativos), tem 48 horas para libertar espaço ou aumentar o seu orçamento antes de os seus Worlds ficarem inacessíveis. Veja [limites de tamanho dos Worlds](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#size-limits) para mais detalhes.

### worlds com múltiplas scenes

Por predefinição, ao fazer deploy para um world via a CLI, a sua scene será tratada como um world de uma única scene. Se o seu world já contiver uma scene, fazer deploy de uma nova scene irá substituir essa scene mais antiga, mesmo que as coordenadas sejam diferentes.

Para fazer deploy como um world com múltiplas scenes, adicione o parâmetro `--multi-scene` ao comando de deploy:

`npm run deploy -- --multi-scene --target-content https://worlds-content-server.decentraland.org`

Ao usar este parâmetro, se a scene que está a publicar tiver coordenadas diferentes das scenes existentes nesse world, essas scenes irão coexistir.


> [!WARNING]
> \*\*📔 Nota\*\*: Se tiver direitos de operador num world que não lhe pertence, e lhe forem concedidos direitos apenas sobre parcels específicos desse world, então só poderá publicar nesses parcels e não poderá afetar o conteúdo de outros parcels do world.


### Optar por não aparecer na listagem do Places

Se quiser optar por não permitir que os seus Worlds sejam indexados no Places, pode adicionar a seguinte secção no seu `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"placesConfig": {
			"optOut": true
		}
	}
}
```

### Usando o Scene Editor no Creator Hub

1. Abra o seu projeto da scene.
2. Clique no **Publicar** botão no canto superior direito.
3. Selecione **PUBLISH TO WORLD**.
4. Selecione para qual dos seus NAMEs ou domínios ENS pretende publicar.

### Via a CLI

Use o seguinte comando:

```bash
npm run deploy -- --target-content https://worlds-content-server.decentraland.org
```

Depois de executar o comando, ser-lhe-á pedido para assinar o deploy com a sua wallet e será executado um conjunto de validações para permitir ou rejeitar a scene.


> [!WARNING]
> **📔 Nota**: Quando a sua `scene.json` tem uma `worldConfiguration` secção, `npm run deploy` precisa da flag `--target-content` (ou `--target`). Sem qualquer uma delas, o comando termina com o erro `DEPLOY_MISSING_TARGET_CONTENT`.


### Aceder a um World

Depois de a scene ser carregada para o servidor de Worlds, pode acedê-la usando o Decentraland Explorer com o seguinte URL: `decentraland://?realm=NAME.dcl.eth`, onde `NAME` deve ser substituído pelo NAME da Decentraland ou domínio ENS para o qual fez deploy.

Com a Decentraland já aberta, também pode ir para um world escrevendo o `/goto NAME.dcl.eth` comando na caixa de chat.

Veja [Tornar descobrível](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/make-discoverable/) para mais informações sobre como tornar o seu World descobrível.

### Migrar um World para a Genesis City

Se for proprietário de LAND e quiser fazer deploy de uma scene de World para a Genesis City, pode fazê-lo ao voltar a fazer deploy da sua scene para a rede descentralizada Catalyst, que é o servidor de conteúdo da Genesis City.

Aspetos a lembrar:

* Remova a `worldConfiguration` secção de `scene.json`
* A limitação de tamanho para Worlds (dinâmica com base nos ativos) é diferente da dos parcels de LAND (15 MB por parcel), por isso certifique-se de que a sua scene tem o tamanho correto para deploys para a Genesis City!

## Publicar scenes grandes

Scenes grandes são carregadas em lotes, pelo que pode publicar scenes com mais de cerca de 200 MB para a Genesis City ou para um World. Isto precisa de `@dcl/sdk` 7.31.0 ou posterior no seu projeto. O Creator Hub publica com o SDK instalado no seu projeto, por isso o mesmo se aplica quando publica a partir do Creator Hub.

Como funciona o upload:

* **Abaixo de 50 MiB** (cerca de 52 MB) de ficheiros no total: a scene é carregada num único request, como antes.
* **50 MiB ou mais**: os ficheiros são carregados em lotes de 32 MiB, três lotes de cada vez. Um ficheiro nunca é dividido entre lotes. Se um ficheiro for maior do que 32 MiB, os lotes aumentam para o acomodar.
* Após cada lote, a CLI imprime uma linha de progresso como `Uploaded 96.0 MB of 268.8 MB (batch 3/9)`.

O carregamento em lotes não altera os limites de tamanho. Cada ficheiro tem de continuar abaixo de 50 MB, e a scene inteira tem de caber no [limite total de tamanho dos ficheiros](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) dos seus parcels ou do seu [orçamento de armazenamento dos Worlds](#worlds-storage-budget).


> [!WARNING]
> **📔 Nota**: Se o servidor para o qual faz deploy não suportar carregamentos em lotes, como pode acontecer com um [servidor personalizado](#custom-servers)rewards server `O servidor de destino não suporta carregamentos em lotes; o upload será feito num único request.` e carrega-os num único request. Se totalizarem mais, o deploy falha com o erro `DEPLOY_PARTIAL_NOT_SUPPORTED`. Um único request não pode exceder 200 MB porque a infraestrutura à frente dos content servers faz com que expire.


### Cancelar um deploy

O separador do browser que abre quando executa `npm run deploy` mostra o progresso do upload, com um **Cancelar** botão. Cancelar interrompe o upload. Se o servidor já tiver publicado a scene quando o cancelamento chegar, o deploy termina como publicado.


> [!CAUTION]
> **❗Aviso**: Quando publica para um World sem `--multi-scene`, e o World tem scenes noutros parcels, a CLI remove essas scenes antes de carregar a sua (veja [worlds com múltiplas scenes](#multi-scene-worlds)). Se cancelar depois de elas terem sido removidas, o World pode ficar vazio, e a CLI termina com o erro `DEPLOY_CANCELLED_AFTER_UNDEPLOY`. Faça deploy novamente para publicar a scene.


Se a CLI não conseguir confirmar se o servidor publicou a scene, termina com o erro `DEPLOY_CANCEL_OUTCOME_UNCERTAIN`. Verifique a scene antes de fazer deploy novamente.

As ferramentas que executam o deploy sem o separador do browser podem cancelá-lo através da API de deploy, com um token. A CLI cria um token aleatório em cada execução. Para definir o seu próprio, passe `--deploy-token` com um valor de pelo menos 16 caracteres:

```bash
npm run deploy -- --deploy-token my-own-deploy-token-0001
```

Com `--programmatic`, o resultado do comando inclui este token como `deployToken`. Execute `npx sdk-commands deploy --help` para ver as rotas da API de deploy.

### Códigos de erro de deploy

| Código de erro                    | O que significa                                                                                                                                                                         |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DEPLOY_PARTIAL_NOT_SUPPORTED`    | O servidor não suporta carregamentos em lotes, e os ficheiros em falta ultrapassam 200 MB.                                                                                              |
| `DEPLOY_CONTENT_CHECK_FAILED`     | A CLI não conseguiu verificar de que ficheiros o servidor já dispõe. Esta verificação só é executada quando o servidor não suporta carregamentos em lotes e a scene tem mais de 200 MB. |
| `DEPLOY_CANCELLED`                | O deploy foi cancelado, e a scene não foi publicada.                                                                                                                                    |
| `DEPLOY_CANCELLED_AFTER_UNDEPLOY` | O deploy foi cancelado depois de as scenes anteriores do World terem sido removidas.                                                                                                    |
| `DEPLOY_CANCEL_OUTCOME_UNCERTAIN` | O deploy foi cancelado, mas a CLI não conseguiu confirmar se o servidor o publicou.                                                                                                     |
| `DEPLOY_MISSING_TARGET_CONTENT`   | A scene tem um(a) `worldConfiguration` mas o comando não tem `--target-content` ou `--target`.                                                                                          |

## Publicar a partir de uma hardware wallet

Em vez de armazenar os seus tokens LAND numa conta Metamask, pode ser mais seguro armazená-los num dispositivo de hardware wallet, como um [Ledger](https://www.ledger.com/) ou um [Trezor](https://trezor.io/), que está fisicamente ligado ao seu computador.

Se estiver a usar um destes dispositivos, pode ligar a hardware wallet ao Metamask para permitir assinar mensagens, mantendo os tokens mais seguros. Veja [este artigo da Metamask](https://metamask.zendesk.com/hc/en-us/articles/360020394612-How-to-connect-a-Trezor-or-Ledger-Hardware-Wallet) para obter instruções para ligar a sua conta.

Assim que a sua hardware wallet puder ser usada via Metamask, pode fazer deploy seguindo os mesmos passos como se os seus tokens estivessem numa conta Metamask.

## Substituição de scene

Quando uma nova scene é publicada, substitui o conteúdo mais antigo que existia nos parcels que ocupa.

Se uma scene que ocupa vários parcels for apenas parcialmente substituída por outra, todos os seus parcels são substituídos ou apagados.

Suponha que publicou a sua scene *A* em dois parcels *\[100, 100]* e *\[100, 101]*. Depois vende o parcel *\[100, 101]* a um utilizador que possui land adjacente e que publica uma scene grande (*B*) em vários parcels, incluindo *\[100, 101]*.

A sua scene *A* não pode ser renderizada parcialmente num único parcel, por isso *\[100, 100]* não apresentará qualquer conteúdo. Tem de construir uma nova versão da scene *A* que ocupe apenas um parcel e publicá-la apenas no parcel *\[100, 100]*.

## O que são os content servers

Os content servers são uma rede de servidores propriedade da comunidade com um filesystem content-addressed, o que significa que cada ficheiro é identificado pelo seu conteúdo, e não por um nome de ficheiro arbitrário.

Usamos os content servers para alojar e distribuir todo o conteúdo das scenes de forma semelhante ao BitTorrent, mantendo a rede da Decentraland distribuída.

1. Os content servers armazenam e distribuem todos os assets necessários para renderizar as suas scenes.
2. O `npm run deploy` o comando associa estes assets ao parcel de LAND especificado no seu **scene.json** file. Sempre que volta a fazer deploy da sua scene, os content servers atualizam os respetivos registos para apontar os parcels para o conteúdo mais recente. Fazer deploy não envolve qualquer transação na blockchain; apenas assina a nova entity da scene com a sua wallet para provar a propriedade.

As informações em cada cópia do servidor são verificáveis, uma vez que cada scene é assinada pelo hash do proprietário do LAND. Isto significa que alguém a alojar uma cópia do servidor não conseguirá adulterar o conteúdo para mostrar algo ilegítimo. A comunidade também pode votar para aprovar ou remover qualquer um destes servidores usando a DAO.

## Servers personalizados

Pode fazer deploy de conteúdo para um servidor personalizado que não pertença à rede oficial de catalyst servers mantida pela DAO. Para isso, não precisa de possuir quaisquer tokens LAND ou NAME, pois pode configurar o servidor para usar qualquer lógica de validação que prefira para controlar quem pode fazer deploy e onde. Os servidores personalizados podem escolher incluir conteúdo dos servidores oficiais (que pode substituir), ou começar do zero e publicar conteúdo totalmente novo.

Veja o [repositório catalyst-owner](https://github.com/decentraland/catalyst-owner) para mais informações sobre o que pode fazer com o seu próprio servidor e como configurá-lo.


> [!WARNING]
> **📔 Nota**: Os players terão de escrever manualmente um URL para aceder ao seu servidor personalizado. Algumas validações de serviços como o [rewards server](/docs/creator/content-creator-pt/recompensas/getting-started/) podem falhar nestes contextos, uma vez que frequentemente estes serviços exigem que o request venha de um servidor oficial.


Os players nunca são redirecionados para este servidor; a única forma de aceder é escrever explicitamente o URL para se ligar a ele.

### Via a CLI

Para fazer deploy para um servidor personalizado, execute:

`npm run deploy -- --target <CUSTOM SERVER DOMAIN>`

O URL para aceder à sua scene publicada no seu servidor personalizado dependerá do domínio onde está alojada.

## Verificar sucesso do deploy

Depois de publicar a sua scene, estas alterações podem demorar alguns minutos a propagar-se pelos vários content servers da rede. Se entrar na Decentraland imediatamente após fazer deploy, poderá ainda ver brevemente a versão anterior do seu conteúdo.

Se usar a CLI para fazer deploy, a dapp de assinatura começará a apresentar confirmações de que a nova versão do seu conteúdo foi propagada por todos os servers da rede.

Verá uma lista de cada um dos servers que compõem a rede de conteúdo da Decentraland. Para cada servidor, é especificado o timestamp da última alteração carregada nesse parcel. Cada servidor corresponde a um realm diferente. Pode ver como estes nomes de server se mapeiam para nomes de realm no [catalyst monitor screen](https://decentraland.github.io/catalyst-monitor/).

A propagação é apenas parte da história: após o upload, os modelos 3D da scene são convertidos em asset bundles otimizados para cada plataforma (Windows e Mac). Até essa conversão terminar, é deliberadamente servida aos players a última versão totalmente funcional da sua scene. Normalmente, isto demora apenas alguns segundos, mas pode demorar mais em scenes muito grandes ou quando os servers estão ocupados. Pode consultar diretamente o progresso da conversão, veja [Verifique o estado da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status).

Você também pode obter as informações de propagação a qualquer momento executando o seguinte comando no console da linha de comando:

`npx @dcl/opscli pointer-consistency --pointer 0,0`


> [!WARNING]
> **📔 Nota**: Use as coordenadas da sua scene em vez de `0,0`. Se sua scene tiver vários parcels, qualquer um dos parcels produzirá a mesma saída. Se as coordenadas começarem com um número negativo, adicione uma `\` no início das coordenadas para evitar que o `-` caractere seja interpretado incorretamente pela linha de comando.


## Deploys automáticos

Se você faz alterações regularmente no conteúdo da sua scene e quer implementar um pipeline de publicação simplificado, você pode automatizar o deployment da sua scene por meio de uma GitHub action.

Para isso, primeiro você deve armazenar sua scene em um projeto no GitHub. Em seguida, você pode configurar uma [GitHub action](https://docs.github.com/en/actions) com o seguinte script, que é executado toda vez que houver um merge para a `main` branch. O script instala quaisquer dependências, faz o build do projeto e depois faz o deploy para Decentraland.

```yaml
name: Deploy to DCL PROD

on:
  push:
    branches:
      - main

env:
  DCL_PRIVATE_KEY: ${{ secrets.DCL_PRIVATE_KEY }}

jobs:
  deploy:
    name: Deploy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Install npm packages
        run: |
          npm install
      - name: Build scene
        run: |
          npm run build
      - name: Deploy scene
        run: |
          npm run deploy
```

> Importante: Para que esse processo seja executado, você deve definir a chave privada de uma Wallet como uma variável de ambiente no GitHub, que é usada para assinar o deployment. Como sempre, seja muito cuidadoso ao manter as chaves privadas seguras. NÃO use a chave privada da conta que realmente possui os tokens LAND, pois isso representaria riscos significativos de segurança. Em vez disso, delegue os direitos de operador a uma Wallet descartável que não possua nenhum token valioso. Se essa chave privada for algum dia vazada, você pode revogar facilmente esses direitos de operador da conta e configurar uma nova Wallet.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/publicacao/publishing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
