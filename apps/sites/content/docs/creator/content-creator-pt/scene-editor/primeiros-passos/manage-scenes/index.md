> Gerenciando seus projetos de scene

# Gerenciar scenes

A **Cenas** O separador lista cada uma das suas cenas disponíveis como um cartão. Abra o cartão para editar essa cena; a partir daí, também pode pré-visualizá-la ou publicá-la.

![O separador Scenes do Creator Hub, mostrando uma grelha de cartões de cena com um cartão Nova Cena em primeiro lugar, os botões Importar Cena e Templates acima da grelha e uma coluna Tutorials à direita.](/docs/_remote/8a4ec6d91bfb06d4402cf305.webp)

## Criar uma cena

Clique na seta ao lado do **Criar** botão e selecione **Nova cena** para criar uma nova cena. Também pode clicar no **Nova cena** cartão no início da lista de cenas.

![O menu suspenso do botão Criar, aberto, com as opções Nova cena e Importar cena.](/docs/_remote/727cb68c3c3feea0e6221431.webp)

Ser-lhe-á pedido que dê um nome à sua cena e que escolha um local para a guardar. A localização predefinida é a pasta Scenes definida nas preferências da aplicação; consulte [Alterar a pasta de cenas](#change-the-scenes-folder).

![A caixa de diálogo Create Project, com um campo Project Name, um campo Project Path com um seletor de pasta e os botões Cancel e Create.](/docs/_remote/b21de226270dbf1e41f5f2ca.webp)

Depois de confirmar estes passos, o projeto da cena será criado. Isto pode demorar um ou dois minutos, uma vez que descarrega dependências e configura uma pasta na sua máquina local com tudo o que precisa. Quando terminar, a sua cena será aberta no [Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/).

### Começar a partir de um template

Para começar com uma cena já pronta em vez de uma vazia, clique em **Modelos** na **Cenas** separador, ou desloque-se até à **Modelos** linha no **Home** separador. Os templates vão desde um **Empty Scene** até cenas completas, como uma galeria de arte ou uma discoteca. Pode filtrá-los por dificuldade. Clique num cartão de template para criar uma nova cena com base nele, depois dê-lhe um nome e escolha uma localização como faria para uma cena vazia.

![O ecrã Choose a Template, com filtros de dificuldade Easy, Medium e Hard e uma grelha de cartões de template, incluindo Empty Scene, Art Gallery Template e Cozy House Template.](/docs/_remote/9f7cb7f9f5985517a1f16c0d.webp)

### Opções do cartão da cena

Clique nos três pontos no cartão de uma cena para ver mais opções.

![Um cartão de cena com o menu de três pontos aberto, listando Duplicate, Open Folder Location, Rename Folder, View Deployments e Delete from My Scenes.](/docs/_remote/def8b7efaebef1e28761a7ce.webp)

Clique em **Duplicar** para fazer uma cópia de uma cena existente.

Para alterar o nome de apresentação da sua cena, abra-a e clique no ícone do lápis para alterar o **Nome** campo e outras propriedades.

Para renomear a pasta da cena no disco, clique nos três pontos no cartão da cena e selecione **Rename Folder**. Introduza o novo nome da pasta e confirme. O nome da pasta tem de ser válido para o seu sistema operativo e não pode colidir com uma pasta existente no mesmo local.

## Importar uma cena

O gestor de cenas apresenta as cenas que encontra no caminho predefinido na sua máquina.

Para adicionar uma cena que esteja noutro local no seu disco local, clique em **Import scene** e encontre o caminho para a pasta do projeto. A cena importada ficará agora disponível como um novo cartão no ecrã do gestor de cenas.

A cena importada não é movida no seu disco local.


> [!WARNING]
> **📔 Nota**: Não renomeie nem mova manualmente a pasta de uma cena importada diretamente a partir do seu gestor de ficheiros. O Scene Editor deixará de conseguir encontrar a cena importada no seu novo caminho.


As cenas que criou no antigo editor web são armazenadas na cloud. Para trabalhar nessas cenas a partir do Scene Editor para desktop, tem de exportar a cena do Web Editor, descompactá-la para uma pasta e depois importá-la no Scene Editor para desktop. Consulte [Migrar do Web Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/migrate-from-web/) para mais detalhes.

## Eliminar uma cena

No ecrã de seleção de cenas, carregue no *três pontos* ícone e selecione *Delete from My Scenes*.

Isto remove a cena do ecrã inicial do seu Scene Editor. Por predefinição, não elimina os ficheiros da sua máquina, mas a caixa de diálogo de confirmação inclui uma caixa de seleção para **eliminar também os ficheiros da cena do seu computador**.

![A caixa de diálogo de confirmação de eliminação, a perguntar se tem a certeza de que pretende eliminar a cena de My Scenes, com uma opção desmarcada para eliminar também os ficheiros da cena do seu computador.](/docs/_remote/710213c338c5781ee7ca3f8f.webp)

## Alterar a pasta de cenas

Por predefinição, os projetos criados através do Scene Editor são mantidos dentro de uma `Cenas` pasta no diretório de dados da aplicação do Creator Hub. Para alterar esta localização:

1. Clique no logótipo do Creator Hub no canto superior esquerdo para abrir o menu principal e depois clique em **Configurações**.
2. No **SCENES** separador, clique no ícone de pasta junto a **Scenes Folder** e escolha uma nova pasta.

![A caixa de diálogo App Preferences no separador SCENES, mostrando o caminho de Scenes Folder com um seletor de pastas e as opções de Scene Dependencies: Auto-update all my scenes, Notify me of new version updates e Do nothing.](/docs/_remote/f3a6d0025165ce6a10282622.webp)

O mesmo separador também controla o que acontece quando está disponível uma nova versão das dependências da cena: atualizar automaticamente todas as suas cenas, receber uma notificação ou não fazer nada.

Para navegar até à pasta de um projeto, clique nos três pontos no respetivo cartão e selecione **Open Folder Location**.

## Gerenciar Worlds

Se for titular de um Decentraland NAME ou de um domínio ENS, pode publicar cenas no seu [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds). Os Worlds aparecem no Scene Editor tal como cenas normais, e pode publicar para eles utilizando o mesmo **Publicar** button.

Um World tem os seus próprios metadados (nome, descrição e miniatura), separados dos metadados de cada cena publicada nele. Em Worlds com uma única cena, mantém-se automaticamente sincronizado com os metadados da cena; em Worlds com várias cenas, só pode ser editado no **Gerir** separador. Consulte [Metadados do World vs metadados da scene](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#world-metadata-vs-scene-metadata).

### Visualizing storage space

As cenas publicadas em Worlds contam para um orçamento de armazenamento partilhado entre todos os Worlds detidos pela sua wallet. O orçamento é calculado com base nas suas posses: cada Decentraland NAME ou parcela de LAND que possui atribui 100 MB, e cada 2.000 MANA mantidos na sua wallet atribui 100 MB adicionais.

Pode verificar o seu orçamento de armazenamento usado e restante em dois locais:

* A **Gerir** secção do Creator Hub mostra quanto do seu orçamento total está usado e qual é a sua capacidade total de armazenamento. Clique em **Ver Detalhes** para ver um detalhamento de como suas posses de MANA, LAND e NAME se somam.

<img src="/docs/_remote/6b3fe18d494c053a67e9cb4f.webp" alt="" width="300">

* A **Worlds** separador do [Builder](https://decentraland.org/builder/worlds).

### Despublicar cenas

Se precisar de libertar espaço de armazenamento, pode despublicar cenas do World Content Server. Isto pode ser feito através da interface do Builder, que permite despublicar facilmente cenas para libertar espaço de armazenamento.

Para titulares de Decentraland NAME, se exceder o espaço de armazenamento atribuído (por exemplo, através de vendas de ativos ou transferências para outra wallet), ser-lhe-á concedida uma janela de 48 horas para resolver a situação. Se não o fizer, os seus Worlds tornar-se-ão inacessíveis após este período de tolerância.

Para recuperar o acesso a um World bloqueado, pode:

* Adquirir mais MANA, Decentraland NAMEs ou LANDs para aumentar a sua capacidade de armazenamento
* Despublicar cenas existentes do World Content Server para libertar espaço de armazenamento

Veja [limites de tamanho dos Worlds](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#size-limits) para informações detalhadas sobre como a capacidade de armazenamento é calculada.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/primeiros-passos/manage-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
