> Como publicar sua scene no LAND ou em um NAME.

# Publicar uma Scene

## Antes de começar

Certifique-se do seguinte:

* Sua scene atende a todas as [limitações da scene](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/). A maioria delas é validada sempre que você executa uma pré-visualização da sua scene.
* Você tem uma [Metamask](https://metamask.io/) conta, com suas parcelas de LAND ou NAME atribuídas a ela.
* Você possui a quantidade necessária de parcelas de LAND adjacentes ou um NAME da Decentraland. Caso contrário, você pode comprar LAND no [Marketplace](https://decentraland.org/marketplace/) ou um NAME no [Marketplace](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> **📔 Nota**: scenes com múltiplas parcelas só podem ser implantadas em parcelas adjacentes.


Verifique sua [scene](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/#scene-details), certifique-se de fornecer um nome atraente, descrição, miniatura, categorias etc.


> [!CAUTION]
> **❗Aviso**: Ao planejar eventos ao vivo, não faça alterações de última hora na scene pouco antes do evento. Publique sua versão final pelo menos uma hora antes do início do evento.
> 
> Os players que já haviam carregado a scene na sessão atual antes das suas atualizações continuarão a ver uma versão em cache da scene. Para ver a versão atualizada, eles devem fechar e entrar novamente no Decentraland.


## Publique sua scene

Para publicar sua scene:

1. Abra sua scene no Scene Editor e clique em **Publicar**. Isso abre uma janela mostrando detalhes sobre a publicação.
2. Selecione se deseja publicar para LAND ou para um World. Veja [Tipos de projetos](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/) para entender melhor as diferentes opções.

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

3. Se publicar para LAND, selecione a localização no mapa. Você verá suas parcelas elegíveis marcadas em vermelho. Se publicar para um World, verá seus NAMEs elegíveis em um dropdown.


> [!NOTE]
> **💡 Dica**: Se você não vir suas parcelas ou NAMEs, certifique-se de estar conectado ao Creator Hub usando a conta de usuário correta. Caso contrário, saia do projeto e clique no ícone de configurações do usuário no canto superior direito, depois selecione **Sair** e entre novamente.


4. A próxima tela mostra todos os arquivos que você está enviando atualmente e seus tamanhos; confirme a operação.
5. O processo de publicação então começará. Ele tem duas etapas e, quando ambas estiverem concluídas, um **Entrar** botão aparece.


> [!NOTE]
> **💡 Dica**: A tela de sucesso também mostra um link da web para a sua scene, com um botão para copiá-lo. Qualquer pessoa pode abrir esse link em um navegador, tenha ou não o cliente para desktop instalado, então é o que deve ser colado em uma mensagem do Discord ou numa página de evento. Ele se parece com `https://decentraland.org/jump?realm=<your-world-name>` para um World, ou `https://decentraland.org/jump?position=<x>,<y>` para uma scene no Genesis City.
> 
> O **Entrar** botão é para você, e não para o seu público: ele abre a scene no seu próprio cliente para desktop para que você possa verificar seu trabalho.


![](/docs/_remote/a76ebe12a85e91368d25d90d.webp)


> [!NOTE]
> **📔 Nota**: As duas etapas da implantação envolvem:
> 
> * **1. Envio**: Enviando os arquivos para os servidores. Scenes de 50 MiB ou mais são enviadas em lotes, veja [Publicar scenes grandes](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/#publish-large-scenes).
> * **2. Convertendo**: Os modelos 3D da scene são compactados em Asset Bundles para uma renderização mais rápida. Isso geralmente leva segundos. Pode demorar mais para scenes muito grandes.
> 
> Os servidores da Decentraland também geram versões de Low Level of Detail (LOD) dos seus modelos, usadas para renderizar sua scene à distância. Isso é executado em segundo plano assim que sua scene já está no ar, então a publicação nunca espera por isso.


## Quanto tempo leva a publicação?

* **Até você poder testá-lo**: de segundos a alguns minutos. Isso escala com o tamanho total da sua scene e com a largura de banda do seu envio. O **Entrar** botão aparece assim que a scene fica jogável, quando as duas etapas estiverem concluídas.
* **Antes de um evento ao vivo**: publique sua versão final pelo menos uma hora antes do início do evento, para deixar margem para qualquer imprevisto.

Os seguintes fatores podem tornar uma publicação mais lenta:

* **Tamanho total da scene**: afeta tanto o envio quanto a conversão.
* **O número e a complexidade de modelos 3D distintos**: os modelos são convertidos um por um.
* **Quão ocupados estão os servidores de conversão**: sua scene pode ficar na fila atrás de outras scenes publicadas mais ou menos ao mesmo tempo.

## Gerenciando Worlds

O Creator Hub permite o gerenciamento de Worlds por meio da **Gerenciar** aba em seu painel principal. A **Gerenciar** aba permite acompanhar e editar Worlds. A partir daqui, você pode editar as configurações, permissões e scenes do World.

### Orçamento de armazenamento do World

Scenes publicadas em Worlds contam para um orçamento de armazenamento compartilhado entre todos os Worlds pertencentes à sua wallet. O orçamento cresce conforme seus ativos: cada NAME da Decentraland ou parcela de LAND que você possui concede 100 MB, e cada 2.000 MANA mantidos em sua wallet concede mais 100 MB. Worlds publicados em domínios ENS têm um limite fixo de 36 MB.

O **Gerenciar** aba mostra quanto do seu orçamento de armazenamento você está usando. Clique em **Ver detalhes** para ver como seus ativos de MANA, LAND e NAME se somam.

<img src="/docs/_remote/6b3fe18d494c053a67e9cb4f.webp" alt="" width="300">

Você também pode verificar seu orçamento restante na **Worlds** aba do [Builder](https://decentraland.org/builder/worlds).

Veja [Limites de tamanho dos Worlds](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#size-limits) para detalhes sobre como o orçamento é calculado e o que acontece se você o exceder.

### Configurações do World

Um World Owner pode editar suas configurações entrando no World desejado **Configurações** no **Gerenciar** painel, ou acessando-o durante o processo de publicação clicando em **Configurações** se **World Multi-Scene (Avançado)** estiver ativado.

<img src="/docs/_remote/c29f510065b494f433726156.webp" alt="" width="600">

* **Detalhes**: informações gerais do World:
  * Título do World
  * Descrição
  * Classificação de conteúdo
  * Categorias

As informações adicionadas em **Detalhes** serão mostradas no Decentraland Places e nas informações do World dentro do World assim que forem publicadas. Tenha em mente que, se o World contiver uma única scene, esses detalhes serão substituídos pelos metadados da scene a cada publicação. Veja [Metadados do World vs metadados da scene](#world-metadata-vs-scene-metadata) abaixo.

* **Layout**: Acessível apenas em Worlds Multi-Scene. Contém informações sobre todas as scenes publicadas do World.
  * Remova scenes individuais clicando nos três pontos e selecionando **Remover do World**.
  * **Mapa do World** mostra o layout do World e identifica as parcelas com conteúdo e as parcelas livres restantes.

<img src="/docs/_remote/f965a7cd250c7251c2c60acb.webp" alt="" width="600">

* **Diversos**: Outras configurações úteis do World:
  * Coordenada de Spawn do World: isso define a Parcel (X,Y) em que o usuário fará spawn dentro do World. A scene localizada nessa Parcel determina a posição exata em que o usuário fará spawn (por exemplo, a Parcel 1,1 é o World Spawn, e a scene em 1,1 tem um ponto de Spawn em 1,0,1 **dentro dessa scene**).
  * Configurações do skybox


> [!NOTE]
> **📔 Nota**: As Configurações do World estão acessíveis apenas ao World Owner (o endereço que fez o mint do NAME). Para mais detalhes sobre como obter um NAME, confira a [seção NAMEs do Marketplace](https://decentraland.org/marketplace/names/claim).


### Metadados do World vs metadados da scene

Um World e cada scene publicada nele têm dois conjuntos separados de metadados. Ambos incluem um nome, uma descrição e uma miniatura:

* **Metadados da scene**: Pertence ao projeto da scene, armazenado em seu `scene.json` arquivo. Você o edita por meio das [configurações da scene](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/) no Scene Editor, e ele é enviado junto com a scene sempre que você publica.
* **Metadados do World**: Pertence ao próprio World. Você o edita nas **Configurações**, no **Gerenciar** aba do Creator Hub. Essas são as informações mostradas no Decentraland Places e nas informações do World dentro do World.

A relação entre esses dois depende de quantas scenes o World contém:

* **World vazio**: Quando você publica uma scene em um World vazio, os metadados do World são preenchidos a partir dos metadados da scene.
* **World de scene única**: Toda vez que você publica a scene, os metadados do World são atualizados para corresponder aos metadados da scene.
* **World de múltiplas scenes**: Os metadados do World são totalmente independentes dos metadados de cada uma de suas scenes. Publicar uma scene nunca altera os metadados do World. A única forma de alterá-los é por meio das **Configurações** no **Gerenciar** aba.

Na prática, se o seu World contiver uma única scene, você não precisa pensar nos metadados do World como algo separado: basta editar as configurações da sua scene e publicar, e o World sempre refletirá isso.


> [!WARNING]
> **📔 Nota**: Em um World de scene única, quaisquer alterações que você fizer na **Configurações** no **Gerenciar** aba serão substituídas pelos metadados da scene na próxima vez que você publicar a scene. Para que essas alterações permaneçam, edite as configurações da scene em vez disso.


Em um World Multi-Scene, a **Configurações** no **Gerenciar** aba é o único lugar para editar os metadados do World.

### Worlds Multi-Scene

Um World pode ter várias scenes, publicadas pelo World Owner ou por outros criadores. Isso permite um ambiente colaborativo em que cada parcela pode ser gerenciada por diferentes Collaborators.

#### Tornando um World Multi-Scene

Um World Owner pode escolher tornar o World Multi-Scene ativando **World Multi-Scene (Avançado)** ao publicar em um World de scene única.

<img src="/docs/_remote/9f653359d5337efaf125df54.webp" alt="" width="600">

Depois que o World Multi-Scene for publicado, o World Owner pode publicar scenes adicionais ou adicionar Collaborators para publicar dentro do World.


> [!NOTE]
> **📔 Nota**: O tamanho de um World Multi-Scene se adapta automaticamente para conter todas as scenes publicadas, crescendo e encolhendo dinamicamente a cada publicação. O espaço deixado entre diferentes scenes no Multi-World é preenchido com environment.


#### Adicionando Collaborators a um World Multi-Scene

No **Gerenciar** painel, um World Owner pode acessar as **Permissões** clicando nos três pontos. O World Owner pode gerenciar colaboradores na **Colaboradores** aba.

<img src="/docs/_remote/2586c8bab86636ee8a2193fe.webp" alt="" width="600">

Um Colaborador pode ter direitos de deploy para Todas as Parcelas ou para Coordenadas Personalizadas específicas. As Coordenadas Personalizadas podem ser selecionadas e confirmadas por meio de um mapa interativo do World, semelhante ao das Configurações do World.

<img src="/docs/_remote/ef6b0e88960c90ce2923d1fe.webp" alt="" width="600">

#### Fazendo deploy em um World Multi-Scene como Collaborator

Os Collaborators do World não podem editar suas Configurações ou Permissões. Na **Gerenciar** aba, um criador pode ver o World em que é Collaborator, mas não pode acessar **Configurações** ou **Permissões**.

<img src="/docs/_remote/f15119b09e3098fe4efe4dbe.webp" alt="" width="600">

Ao passar pelo processo de publicação, o criador pode escolher publicar apenas nas parcelas em que é Collaborator (conforme definido pelo World Owner).

No **Colaboradores** seção, se o World Owner tiver definido **Coordenadas personalizadas** para o criador, apenas as parcelas atribuídas estarão disponíveis para publicação. Se o acesso foi definido como **Todas as Parcelas**, o criador poderá selecionar qualquer parcela no World para publicar sua scene.


> [!WARNING]
> **📔 Nota**: Colaboradores com **Todas as Parcelas** acesso de publicação podem sobrescrever qualquer scene do World, mesmo que tenha sido publicada pelo proprietário ou por outros collaborators.


<img src="/docs/_remote/4a5bbf90cf2c56710137c17e.webp" alt="" width="600">

### Worlds privados

Um WORLD pode ter diferentes **Acesso** configurações. Pode ser acessível a qualquer pessoa ou ser restrito de diferentes maneiras.

#### Definindo o acesso de um WORLD

No **Gerenciar** painel, um World Owner pode acessar as **Permissões** clicando nos três pontos. O World Owner pode gerenciar as restrições de acesso na **Acesso** aba.

**Tipos de acesso**

Um World Owner pode escolher entre três tipos de **Acesso ao World**:

**Público**

Qualquer pessoa pode acessar o World. Esta é a configuração padrão de um World.

**Protegido por senha**

Somente usuários com a senha podem entrar no World.

As senhas devem ter pelo menos 8 caracteres e conter pelo menos 2 números. Depois de criada, a senha não poderá ser acessada, então certifique-se de guardar uma cópia.

**Somente por convite**

Somente endereços e Comunidades adicionados em **Endereços aprovados** podem acessar o World.

Para adicionar novos endereços ou comunidades ao **Endereços aprovados**, siga estas etapas:

1. Clique no **+ Novo convite** botão.
2. Você pode adicionar endereços de três maneiras diferentes:

* **Endereço da wallet**: Adicione wallets individuais, uma de cada vez.
* **Comunidade**: Pesquise e adicione qualquer Comunidade pública. Isso adiciona **todos os endereços da Community** à **Endereços aprovados**.

  <img src="/docs/_remote/cd2c518c4a9642b70cf0c060.webp" alt="" width="600">
* **Importar CSV**: Use um CSV existente com uma lista de endereços ou IDs de comunidades para adicionar a **Endereços aprovados**. A estrutura é uma wallet por linha, por exemplo:

```
0x3bA7fD92eC4a1F6B8d2E9c5A7b1D3f6C8e4A2d9F
0xA1c9E4b7D2f6C8a3B5e9F1d4A7c2E6b8D3f9C5a1
```

Depois de importado, ele rastreia cada Address individualmente, como mostrado na imagem.

<img src="/docs/_remote/554211db6a545dd6cf7ee848.webp" alt="" width="600">

3. Após confirmar, o(s) endereço(s) estão em **Endereços aprovados**.
4. Com um novo **+ Novo convite**, os endereços são adicionados à lista existente, ajudando o World Owner a gerenciar e ampliar a lista, se necessário.

<img src="/docs/_remote/c316240a3038949beab28744.webp" alt="" width="600">

5. Endereços individuais ou conjunto de endereços (no caso de uma Community) podem ser removidos selecionando **Excluir** nos três pontos na **Endereços aprovados** seção.


> [!WARNING]
> **📔 Nota**: Se você alterar o **Acesso** tipo de **Somente por convite**, sua **Endereços aprovados** lista será removida. Certifique-se de ter uma cópia caso precise dela no futuro.


#### Entrando em Worlds privados

Há diferentes cenários se um usuário entrar em um World que não tenha **Público** acesso:

* O endereço deles na **Endereços aprovados**: Poderá entrar normalmente. Caso contrário, receberá a informação de que o World está **Somente por convite**.

<img src="/docs/_remote/ac4dca8cd154a2b6ce27d48d.webp" alt="" width="300">

* O World está **Protegido por senha**: Os usuários poderão digitar a senha. O limite máximo é de dez (10) tentativas.

<img src="/docs/_remote/65d948411619e496f8eed2f7.webp" alt="" width="300">

## Publicar a partir de uma wallet de hardware

Em vez de armazenar seus tokens LAND em uma conta Metamask, você pode achar mais seguro armazená-los em um dispositivo de wallet de hardware, como um [Ledger](https://www.ledger.com/) ou um [Trezor](https://trezor.io/), conectado fisicamente ao seu computador.

Se você estiver usando um desses dispositivos, pode vincular a wallet de hardware ao Metamask para permitir a assinatura de mensagens, mantendo os tokens mais seguros. Veja [este artigo do Metamask](https://support.metamask.io/more-web3/wallets/how-to-connect-a-trezor-or-ledger-hardware-wallet/) para instruções de conexão da sua conta.

Depois que sua wallet de hardware puder ser usada via Metamask, você poderá fazer deploy seguindo os mesmos passos de uma conta Metamask.

## Sobrescrita de scene

Quando uma nova scene é feita deploy, ela sobrescreve o conteúdo antigo que existia nas parcelas que ocupa.

A publicação normalmente começa imediatamente. Se você estiver prestes a substituir conteúdo que já existe em um World, o Creator Hub pede que você confirme primeiro e oferece uma **Não mostrar novamente** opção.

Se uma scene que ocupa várias parcelas for apenas parcialmente sobrescrita por outra, todas as suas parcelas serão sobrescritas ou apagadas.

Suponha que você tenha feito deploy da sua scene *A* em duas parcelas *\[100, 100]* e *\[100, 101]*. *\[100, 101]* Então você vende a parcela*para um usuário que possui LAND adjacente e que faz deploy de uma scene grande (*&#x42; *\[100, 101]*.

Sua scene *A* não pode ser renderizada parcialmente em apenas uma parcela, então *\[100, 100]* não exibirá nenhum conteúdo. Você deve criar uma nova versão da scene *A* que ocupe apenas uma parcela e fazer deploy apenas na parcela *\[100, 100]*.

## Publicar em LAND concedido

Se você estiver publicando em LAND pertencente à Decentraland Foundation que foi concedido a você por meio de uma grant, clique no **Publicar** botão normalmente e depois selecione **Publicar em um servidor diferente** na parte inferior. Depois selecione **Servidor personalizado** no dropdown e insira o seguinte endereço do servidor: `https://linker-server.decentraland.org`.


> [!WARNING]
> **📔 Nota**: Primeiro você deve definir manualmente as coordenadas da sua scene na aba avançada das configurações de Layout. Veja [Configurações da scene](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/#layout) para mais informações.


## Servidores personalizados

Você pode fazer deploy de conteúdo para um servidor personalizado que não pertença à rede oficial de servidores catalyst mantida pela DAO. Para isso, você não precisa possuir tokens LAND ou NAME, pois pode configurar o servidor para usar qualquer lógica de validação que preferir para controlar quem pode fazer deploy e onde. Servidores personalizados podem optar por ter conteúdo dos servidores oficiais, que você pode sobrescrever, ou começar do zero e publicar conteúdo totalmente novo.

Para publicar em um servidor personalizado, clique no **Publicar** botão normalmente e depois selecione **Publicar em um servidor diferente** na parte inferior. Depois selecione **Servidor personalizado** no dropdown e insira o endereço do servidor.

Veja [Como executar seu próprio Catalyst Node](/docs/contributor/tutorials/how-to-run-a-catalyst/) para mais informações sobre o que você pode fazer com seu próprio servidor e como configurá-lo.


> [!WARNING]
> **📔 Nota**: Os players precisarão digitar manualmente uma URL para acessar seu servidor personalizado. Certas validações de serviços como o [rewards server](/docs/creator/content-creator-pt/recompensas/getting-started/) podem falhar nesses contextos, pois muitas vezes esses serviços exigem que a solicitação venha de um servidor oficial.


Os players nunca são direcionados para este servidor; a única forma de acessá-lo é digitar explicitamente a URL para se conectar a ele.

## Verifique o sucesso do deploy

Depois que você fizer deploy da sua scene, essas alterações podem levar alguns minutos para serem enviadas e processadas. Se você entrar no Decentraland logo após fazer deploy, talvez ainda veja brevemente a versão anterior do seu conteúdo.

O Creator Hub exibe o progresso da publicação à medida que ela passa pelas **Enviando** e **Convertendo** etapas e mostra um **Entrar** botão assim que a scene estiver jogável.

### Verifique o status da conversão

Enquanto os modelos 3D de uma nova versão estão sendo convertidos, os players recebem deliberadamente a última versão totalmente funcional da sua scene. As conversões devem levar no máximo alguns minutos; se demorar mais, pode ser sinal de que algo falhou.

Você pode consultar o status da conversão diretamente em um navegador:

* `https://asset-bundle-registry.decentraland.org/entities/status/<pointer>`: substitua `<pointer>` por uma das coordenadas da sua scene (por exemplo `20,-34`) `assetBundles`, e o status dos LODs em `lods`. Quando `completo` é `true`, a scene está totalmente convertida para todas as plataformas. Para uma scene em um World, adicione o nome do World como parâmetro de consulta, por exemplo `.../entities/status/0,0?world_name=myname.dcl.eth`.
* `https://asset-bundle-registry.decentraland.org/queues/status`: lista os IDs de todas as scenes que estão aguardando para ser convertidas, por plataforma. Se o seu entity ID aparecer aqui, sua scene está na fila atrás de outras conversões e você só precisa esperar um pouco mais.


> [!WARNING]
> **📔 Nota**: Se você tiver uma sessão aberta do Decentraland Explorer na qual já carregou sua scene, precisará fechar e reabrir o aplicativo. Caso contrário, você continuará vendo a versão em cache.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/publicar/publish-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
