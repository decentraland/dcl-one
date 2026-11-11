> Como pode depurar a sua cena enquanto a executa localmente na pré-visualização

# Depurar na pré-visualização

Executar uma pré-visualização fornece algumas informações úteis de depuração e ferramentas para ajudar você a entender como a cena é renderizada.

Se a cena não puder ser compilada, você verá apenas o chão vazio, sem nada renderizado nele. Se isso ocorrer, há vários lugares onde você pode procurar mensagens de erro para ajudar a entender o que deu errado:

1. Verifique o seu editor de código para garantir que ele não marcou nenhum erro de sintaxe ou de lógica.
2. Verifique o console na janela de preview para quaisquer mensagens de erro. Abra o console clicando no ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) ícone no canto superior direito.
3. Se você executou a cena usando o Creator Hub, verifique a janela com a saída do console da cena. Se você executou a cena usando a CLI, verifique a saída da linha de comando onde você executou `npm run start`


> [!NOTE]
> **💡 Dica**: Antes de publicar sua cena no ambiente de produção, você pode publicá-la em um [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) para testá-la em um contexto muito mais próximo da produção.


## Use o console

Abra o console clicando no ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) ícone no canto superior direito.

Você também pode alterná-lo pressionando a tecla de crase no teclado: **`**. Essa tecla fica à esquerda da tecla 1 na maioria dos teclados em inglês. Ou então pressione Shift + **\`** para abrir uma visualização mais ampla do console, caso precise ver mais texto.

Exiba mensagens no console (usando `console.log()`). Em seguida, você pode visualizar essas mensagens à medida que são geradas ao abrir o console da cena.

O console também mostra mensagens de erro; elas são marcadas em vermelho.

## O painel de depuração

Abra o console de depuração clicando no ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) ícone no canto superior direito.

Isso abre um painel no canto inferior direito com informações sobre o mecanismo de renderização, e é atualizado em tempo real à medida que as coisas mudam.

![](/docs/_remote/eace962a130964afb6774cff.webp)

### Cena atual

O **CENA ATUAL** aba inclui informações sobre a cena em que você está atualmente. Isso inclui:

* O número de parcelas na cena
* A altura máxima em que você pode construir (330 metros para cada cena)

Você também pode verificar a **Mostrar limites da cena** opção para ver paredes vermelhas ao redor da cena, que marcam os limites da cena.

### Memória

O **MEMÓRIA** a seção exibe a memória usada por Decentraland. Tente sempre manter esse valor abaixo do **Limiares do orçamento de memória**. Se esse valor crescer demais, há o risco de travar o app para os jogadores que visitam sua cena. Tenha em mente que, em produção, a cena pode ser carregada junto com outras cenas e jogadores ao redor, o que pode contribuir para aumentar o consumo de memória além do que você vê na pré-visualização.

### Performance

O **DESEMPENHO** aba inclui vários campos relacionados a *FPS* (Frames Per Second). Esse valor tende a ser mais baixo à medida que a cena ganha complexidade; certifique-se de mantê-lo sempre acima de 25 FPS, para garantir uma boa experiência para seus jogadores. O FPS varia dependendo da máquina em que você executa isso.

O valor mais importante aqui é o **FPS médio**, que calcula a média do FPS dos últimos 1000 frames.

![](/docs/_remote/5ba9631495e357c71fac976e.webp)


> [!WARNING]
> **📔 Nota**: O FPS do renderer não é o mesmo que as atualizações por segundo do game loop da cena. A lógica da cena tem um limite de 30 FPS, enquanto o FPS do renderer pode chegar a 60 ou mais.


Travadas referem-se a pausas momentâneas na taxa de quadros, que podem ter pouco impacto no número geral de FPS, mas podem ser muito perceptíveis para o jogador. Isso normalmente pode ocorrer quando assets pesados são carregados ou algo semelhante.

### Solicitações Web

O **SOLICITAÇÕES WEB** a seção permite abrir a aba Network do Chrome Devtools, que exibe o número de solicitações web feitas pela cena e o conteúdo delas. Isso é útil para saber se a cena está fazendo solicitações demais aos servidores, o que pode impactar o desempenho da cena.

Se a sua cena interagir com um servidor de terceiros, muitas vezes é útil ver quais dados entram e saem. Clique **Abrir o Chrome Devtools** para abrir uma nova janela do Chrome com a aba Network aberta.


> [!WARNING]
> **📔 Nota**: Para usar este recurso, você primeiro precisa instalar o [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/), pois as dependências necessárias vêm incluídas nessa instalação.
> 
> Você não precisa manter o Creator Hub aberto nem executar a preview da sua cena pelo Creator Hub para usar este recurso; ele também pode ser usado se você estiver usando a CLI para executar a preview da sua cena.


Como alternativa, há várias ferramentas gratuitas que você pode executar na sua máquina e que informam todos os dados que entram e saem da aplicação Decentraland. Por exemplo [Charles](https://www.charlesproxy.com/) ou [Wireshark](https://www.wireshark.org/).

## Recarregamento rápido

Sempre que você fizer alterações na cena, a preview recarrega e é atualizada automaticamente, então não há necessidade de abrir e fechar a janela de preview.

Se precisar recarregar a cena em que você está, clique no **Recarregar cena** botão no canto superior esquerdo da janela de preview.

![](/docs/_remote/a2ec4266edb8ce7bb450b24d.webp)

Você também pode recarregar a cena escrevendo o seguinte no chat e pressionando Enter:

`/reload`

## Reportar um bug

Se você encontrar um problema que não seja com sua cena, mas sim com o Decentraland SDK em geral, consulte [Reportar um bug](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/).

## Versões das dependências

Certifique-se de usar sempre as versões mais recentes de todas as dependências da sua cena, pois quaisquer problemas que você esteja enfrentando já podem ter sido corrigidos em versões mais novas. O Creator Hub notificará você quando houver atualizações para instalar na sua cena do Decentraland.

![](/docs/_remote/a002cdf10ca3a37130f2959d.webp)

Você também pode atualizar manualmente as dependências pela linha de comando. Executar uma cena do Decentraland localmente depende de duas bibliotecas principais: `@dcl/sdk` e `@dcl/js-runtime`, que é instalada em cada pasta do projeto. Certifique-se de que ambas estejam atualizadas. Você pode executar os seguintes comandos para atualizar manualmente essas duas bibliotecas para a versão estável mais recente:

```
npm i @dcl/js-runtime@latest
npm i @dcl/sdk@latest
```

Se a sua cena usa smart items, ela também pode estar usando a `@dcl/asset-packs` biblioteca, que você pode atualizar por meio de:

```
npm i @dcl/asset-packs@latest
```

Se você estiver usando alguma das [bibliotecas utilitárias](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) certifique-se de que elas também estejam atualizadas, pois versões mais antigas dessas bibliotecas podem não ser compatíveis com versões mais novas de `@dcl/sdk`.

## Condições de iluminação

A hora do dia no mundo tem um grande impacto em como os modelos 3D aparecem. A cor da fonte de luz muda sutilmente, com um tom azulado à noite e um tom avermelhado durante o nascer e o pôr do sol. A direção da luz também se move pelo céu, lançando sombras em direções diferentes.

Verifique se a sua cena fica boa em todos os horários do dia alterando o relógio do jogo para valores diferentes. Clique no **hora do skybox** ícone no painel esquerdo da tela para definir qualquer horário de sua preferência. Se esse controle deslizante estiver esmaecido, certifique-se de que a **Auto** opção esteja desativada.

<figure><img src="/docs/_remote/be169c2886b8a8e49dfe5d5d.webp" alt="Dynamic skybox" width="300"><figcaption><p>Instruções no mundo</p></figcaption></figure>

Os materiais do seu modelo 3D podem não parecer iguais aos do tool de modelagem com o qual você o criou. Isso é esperado, pois todos os mecanismos de renderização 3D têm diferenças sutis em como lidam com luz e materiais.

Você também pode configurar sua cena para que a hora do dia seja fixa e a iluminação não mude. Veja [Controle do skybox](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/skybox-control/) para mais detalhes.

## Teste multiplayer

Se você iniciar uma preview da cena e abri-la em duas (ou mais) janelas diferentes do Explorer, cada janela aberta será interpretada como um jogador separado, e um servidor de comunicações simulado manterá esses jogadores sincronizados.

Interaja com a cena em uma janela e depois mude para a outra para ver que os efeitos dessa interação também estão visíveis lá.

Usando o Creator Hub, ative **Preview de múltiplas instâncias** em **cliente de desktop** submenu de **Opções de Play**, depois clique no **Play** botão uma segunda vez para abrir uma segunda janela do Explorer do Decentraland. Você deve conectar as duas janelas com endereços diferentes. As mesmas sessões permanecerão abertas enquanto a cena recarrega. Talvez seja necessário sair e entrar novamente na segunda janela para selecionar uma conta diferente.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Ambos os jogadores verão um ao outro e poderão interagir entre si. Se a cena tiver qualquer lógica multiplayer, ela funcionará como esperado e será sincronizada entre os dois jogadores.

Como alternativa, você pode abrir uma segunda janela do Explorer do Decentraland escrevendo o seguinte em uma URL do navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Usando a rede de teste Ethereum

Ao testar sua cena, para evitar transferir MANA real ou outras moedas, você pode usar a rede de teste Sepolia da Ethereum e transferir MANA de testnet fictício em vez disso.

Para usar a rede de teste, você deve configurar sua extensão Metamask no Chrome para usar a *rede de teste Sepolia* em vez da *rede principal*. Você também deve possuir MANA na blockchain Sepolia, que você pode adquirir gratuitamente no Decentraland.


> [!NOTE]
> **💡 Dica**: Para executar a transação de transferir MANA Sepolia para sua wallet, você precisará pagar uma taxa de gas em Ether Sepolia.


Quaisquer transações que você aceitar enquanto visualiza a cena neste modo ocorrerão apenas na rede de teste e não afetarão o saldo de MANA na sua wallet real.

Para visualizar sua cena usando a rede de teste, cole a seguinte URL em uma aba do navegador. Isso abrirá a cena no cliente de desktop do Decentraland:

> decentraland://realm=<http://127.0.0.1:8000\&local-scene=true\&debug=true\&dclenv=zone\&position=0,0>


> [!NOTE]
> **💡 Dica**: Altere o parâmetro position para as coordenadas da sua cena, para carregar diretamente na sua cena.. Quaisquer transações que você aceitar enquanto visualiza a cena neste modo ocorrerão apenas na rede de teste e não afetarão o saldo de MANA na sua wallet real.


Se você precisar testar transações na Polygon Testnet e precisar ter MANA nessa testnet, será necessário fazer o swap de MANA para essa rede depois de adquiri-lo em Sepolia. Para fazer a bridge de MANA Sepolia para a Polygon Testnet, visite a página da sua conta Decentraland em Sepolia e clique em 'swap' no lado do MANA Ethereum.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-preview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
