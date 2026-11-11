> Crie e altere a sua cena ao conversar com um assistente de IA dentro do Creator Hub.

# Assistente de IA

O Creator Hub pode executar um assistente de IA que vê sua scene aberta e a edita para você. Peça para ele colocar itens, conectar comportamentos, escrever scripts ou executar a preview e verificar o resultado.


> [!WARNING]
> **📔 Nota**: O assistente executa uma CLI de programação que você instala e na qual faz login, com acesso total à sua máquina, o mesmo alcance que ele tem no seu terminal. Ative-o apenas se você confiar no que está pedindo para ele fazer.


## Onde ficam suas settings

O assistente vem ativado por padrão. Suas settings têm sua própria tab:

1. Clique no logotipo do Creator Hub no canto superior esquerdo e selecione **Definições**.
2. Vá para a **AI** aba.

Essa tab contém o **Enable AI Assistant** switch, a opção **Bill this to an API key instead** a opção, a **Connect** lista de coding CLIs e as **Expose AI assistant MCP server** settings.

Quando o assistente está ligado, um **AI Assistant** botão (o ícone de brilhos) aparece na barra superior do editor, à esquerda do **Code** botão. Clique nele para abrir o painel de chat ao lado da sua scene.

![A barra superior do editor, com o nome da scene à esquerda e o ícone de brilhos do AI Assistant, e os botões Code, Preview e Publish à direita.](/docs/_remote/97b2496489d20fd6596ad9b5.webp)

![O painel AI Assistant quando aberto pela primeira vez: um menu suspenso New chat e um menu suspenso CLI mostrando Claude na parte superior, uma mensagem de boas-vindas, um card About AI usage, um chip Selection com o nome da entidade selecionada e o campo Ask Assistant na parte inferior.](/docs/_remote/dceaca65cd87dab45134934a.webp)

## Conectar uma coding CLI

O assistente não usa uma API key própria. Ele executa uma CLI de programação que você instala e na qual faz login, então o trabalho é cobrado na assinatura da própria ferramenta.

Quatro são compatíveis: Claude, Codex, Cursor e Gemini. Você instala cada um com o instalador do próprio fornecedor, a partir de um terminal.

### Instalar uma CLI

Execute o comando para o seu sistema operacional:

| CLI    | macOS e Linux                                           | Windows                                                                                      |
| ------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Claude | `curl -fsSL https://claude.ai/install.sh \| bash`       | `winget install Anthropic.ClaudeCode`                                                        |
| Codex  | `curl -fsSL https://chatgpt.com/codex/install.sh \| sh` | `powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 \| iex"`   |
| Cursor | `curl https://cursor.com/install -fsS \| bash`          | `powershell -ExecutionPolicy ByPass -c "irm 'https://cursor.com/install?win32=true' \| iex"` |
| Gemini | `npm install -g @google/gemini-cli`                     | `npm install -g @google/gemini-cli`                                                          |

Você não precisa copiar isso manualmente. O Creator Hub mostra os comandos exatos para o seu sistema operacional em dois lugares, junto com um link para o guia de instalação desse fornecedor:

* O card de configuração no painel de chat, que aparece na primeira vez que você abre o painel sem nenhuma CLI conectada.
* As settings, na **AI** tab em **Connect**, na **Via Terminal** seção abaixo de cada CLI. Cada comando ali tem um botão **Copy** .

Os guias dos fornecedores:

* [Guia de instalação do Claude Code](https://code.claude.com/docs/en/terminal-guide)
* [Guia de instalação do Codex CLI](https://developers.openai.com/codex/cli)
* [Guia de instalação do Cursor CLI](https://cursor.com/docs/cli/installation)
* [Guia de instalação do Gemini CLI](https://www.geminicli.com/docs/get-started/installation)

Depois de instalar, clique em **Recheck** no card de configuração do painel de chat para que o Creator Hub detecte a nova CLI, sem necessidade de reiniciar o app. A **AI** tab verifica novamente sozinha toda vez que você a abre.


> [!WARNING]
> **📔 Nota**: O Claude Desktop não é a mesma coisa que o Claude Code. O assistente precisa do Claude Code, a ferramenta de terminal da Anthropic, e fazer login no app desktop não faz login nele. Se o Creator Hub encontrar o Claude Desktop sem o Claude Code, ele informa isso e mostra o comando de instalação.
> 
> O Gemini CLI precisa de uma Gemini API key paga ou de uma licença Gemini Code Assist Standard ou Enterprise.


### Fazer login

| CLI    | Fazer login                           |
| ------ | ------------------------------------- |
| Claude | `claude auth login`                   |
| Codex  | `codex login`                         |
| Cursor | `cursor-agent login`                  |
| Gemini | execute `gemini` e siga as instruções |

Execute o comando de login em um terminal. Ele termina no seu navegador.

O Codex tem um atalho: se ele ainda não estiver instalado, o Creator Hub pode instalá-lo e fazer login para você. Clique em **Fazer login com o Codex** no card de configuração do painel de chat, ou **Fazer login** ao lado de Codex em **Connect** na **AI** tab. O Creator Hub executa o login e abre o seu navegador, e um link **Sign out** aparece então no mesmo lugar. Se você instalou o Codex por conta própria com o comando acima, não há botão: faça login em um terminal como os outros.

Um ponto verde e **Connected** ao lado de uma CLI significa que ela está pronta para uso.


> [!NOTE]
> **💡 Dica**: Se você tiver mais de uma CLI conectada, um menu suspenso no painel de chat permite alternar entre elas.


### Quando um agente pede para você fazer login novamente

Um login salvo pode expirar enquanto a CLI ainda parece conectada. Quando uma CLI responde a uma mensagem com "not logged in", o Creator Hub mostra um estado **Sign-in needed** com um ponto âmbar em vez de repassar o comando bruto para o chat.

O painel de chat e a **Via Terminal** seção na **AI** tab mostram os dois comandos para executar, um de cada vez, terminando no seu navegador:

| CLI    | Faça sign out e depois faça login novamente                   |
| ------ | ------------------------------------------------------------- |
| Claude | `claude auth logout`, depois `claude auth login`              |
| Codex  | `codex logout`, depois `codex login`                          |
| Cursor | `cursor-agent logout`, depois `cursor-agent login`            |
| Gemini | execute `gemini`, depois `/auth logout`, depois `/auth login` |

Se o Creator Hub instalou o Codex para você, use **Sign out** e depois **Fazer login** na **AI** tab em vez disso.

Envie sua mensagem novamente assim que estiver conectado. O agente permanece ativado o tempo todo, então você não precisa configurá-lo do zero.

### Cobrar a uma API key em vez disso

Se você preferir pagar por token em vez de usar uma assinatura de CLI, ative **Bill this to an API key instead** na **AI** tab das settings. Ela lê a API key do seu provedor a partir do seu ambiente: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CURSOR_API_KEY` ou `GEMINI_API_KEY`. Isso vem desativado por padrão.

Isso também é uma forma de contornar um login que você não consegue concluir. Quando uma CLI está no estado **Sign-in needed** O Creator Hub mostra um **Ou use uma API key** hint com o nome da variável correspondente.

## O que ele pode fazer

O assistente trabalha na scene que você tem aberta. Ele pode:

* **Editar a scene**: criar e deletar entidades, alterar components, colocar [smart items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/), anexar [scripts](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/), e ler e escrever `scene.json`.
* **Escrever código** nos arquivos da sua scene.
* **Executar a preview** e controlá-la: tirar screenshots, andar por aí, clicar em coisas, ler os logs e verificar o desempenho.
* **Ler sua seleção**, para que você possa selecionar uma entidade e dizer "deixe esta mais alta".

As alterações são salvas à medida que acontecem e entram no histórico normal de undo. Para desfazer uma troca inteira de uma vez, use **Undo AI changes** na resposta do assistente.

![O painel AI Assistant encaixado à direita do editor depois de um pedido para renomear uma entidade e adicionar um Mesh Renderer: a resposta lista as ferramentas que ele executou, descreve o resultado e mostra um botão Undo AI changes abaixo.](/docs/_remote/6a907fb6c14f96a55c037af0.webp)

Enquanto o assistente trabalha, suas alterações aparecem ao vivo na entity tree e no canvas.

![O Scene Editor completo com o painel AI Assistant encaixado à direita, a entidade Stage renomeada visível na entity tree e a resposta do assistente terminando com Undo AI changes.](/docs/_remote/1a8e6565c0eaf9d6cba56d1b.webp)

## Trabalhando com o chat

* **New chat** inicia uma conversa nova.
* **Chat history** lista conversas anteriores com há quanto tempo foram executadas. As conversas são salvas por projeto, então reabrir uma scene traz os chats de volta.
* **Open in a separate window** destaca o assistente em sua própria janela, o que é útil em um segundo monitor. **Dock back in the editor** o devolve.
* O painel pode ser arrastado para ficar mais largo ou mais estreito.

Um **About AI usage** card lembra que o assistente consome tokens da sua conta conectada. Fique de olho no uso dessa conta.

O assistente executa uma mensagem por vez. Enquanto uma resposta está em andamento, um **Thinking…** indicador permanece até a vez terminar, inclusive durante chamadas longas de ferramenta. Enviar uma segunda mensagem não cancela a primeira: espere a resposta ou pressione **Stop** para encerrar a vez você mesmo.

Algumas respostas fazem uma pergunta antes de continuar. Se você deixar uma sem resposta por muito tempo, ela fecha com "This question is no longer active." e você pode perguntar novamente.

### Anexar arquivos a um prompt

Às vezes é mais fácil mostrar algo ao assistente do que descrevê-lo. Você pode anexar arquivos a qualquer mensagem de três maneiras:

* Clique no ícone de clipe de papel à esquerda da caixa de mensagem, rotulado **Attach files**e escolha um ou mais arquivos.
* Arraste arquivos do seu explorador de arquivos para a caixa de mensagem. Um **Drop files to attach** hint aparece enquanto você arrasta.
* Cole uma imagem diretamente da sua área de transferência, por exemplo uma screenshot que você acabou de tirar.

Cada arquivo anexado aparece como um chip acima da caixa de mensagem, com um ícone para o seu tipo: image, modelo 3D (`.glb`, `.gltf`), áudio (`.mp3`, `.wav`, `.ogg`, `.m4a`, `.flac`), ou um arquivo genérico para qualquer outra coisa. Clique no cruz **Remove** em um chip para removê-lo antes de enviar. Você pode anexar até 8 arquivos a uma única mensagem e pode enviar anexos sem nenhum texto.

Os arquivos que você arrasta ou escolhe do disco são entregues ao assistente pelo caminho real, então nada é copiado e não há limite de tamanho. O assistente pode ver imagens diretamente e pode referenciar um modelo ou arquivo de áudio pelo caminho quando escreve o código da scene. Imagens coladas não têm arquivo no disco, então o Creator Hub as salva primeiro em um arquivo temporário; esses são limitados a 8 MB cada.

Se sua CLI do Claude estiver muito antiga, o painel mostra um aviso. Execute `claude update` em um terminal para corrigir isso.

### Aponte para um asset no seu prompt

Anexar um arquivo envia o conteúdo dele. Mencionar um asset é mais leve: isso apenas diz ao assistente qual asset no seu projeto você quer dizer, então você pode dizer "use este" em vez de digitar um caminho.

Com o assistente aberto, dê um clique duplo em qualquer asset na tab **Local Assets** . Ele aparece como um chip acima da caixa de mensagem, mostrando **Asset:&#x20;*****filename***, ou **Built-in animation:&#x20;*****name*** de uma das animações de avatar integradas.

* Mencione quantos assets quiser. Cada um recebe seu próprio chip e seu próprio **Remove** cruz.
* As menções ficam ao lado do chip **Selection** , então você pode apontar para uma entidade na sua scene e para um asset ao mesmo tempo: "coloque esta animação naquele NPC".
* As menções se aplicam apenas à sua próxima mensagem. Elas são limpas quando você a envia.


> [!NOTE]
> **💡 Dica**: Selecionar a raiz da Scene conta como selecionar nada, então ela nunca é enviada ao assistente. Selecione uma entidade real quando quiser que o assistente aja sobre ela.


## Decentraland skills são instaladas para você

Quando o assistente é executado, o Creator Hub baixa as oficiais [Decentraland SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) e as vincula ao seu projeto, para que o assistente siga padrões verificados do SDK7 em vez de adivinhar.

Elas são vinculadas em `.claude/skills` e `.agents/skills` dentro da pasta do seu projeto, e ambas são adicionadas a um `.gitignore` para você, então nunca acabam no seu repo.

## Arquivos que o assistente deixa na pasta da sua scene

O assistente pode ler e editar sua scene aberta por meio de um servidor MCP local. Cada CLI é informada sobre esse servidor de forma diferente, então, dependendo de qual você usa, você pode encontrar novos arquivos no seu projeto:

| CLI    | O que aparece na pasta da scene                                          |
| ------ | ------------------------------------------------------------------------ |
| Cursor | `.cursor/mcp.json`, além de `.cursor/rules/creator-hub.mdc`              |
| Gemini | `.gemini/settings.json`                                                  |
| Claude | Nada. A configuração vai para um arquivo temporário fora do seu projeto. |
| Codex  | Nada. A configuração é passada na linha de comando.                      |

Se você já tiver um desses arquivos de configuração, o Creator Hub mescla a entrada dele em vez de sobrescrever os seus próprios servidores.


> [!WARNING]
> **📔 Nota**: `.cursor/mcp.json` é o único arquivo que contém o próprio token de acesso, porque o formato de configuração do Cursor não tem como lê-lo do ambiente. O Creator Hub o adiciona ao seu `.gitignore` antes de escrevê-lo, então não faça commit dele nem remova essa linha. O arquivo do Gemini referencia o token por meio de uma variável de ambiente `$CREATOR_HUB_MCP_TOKEN` em vez disso, então ele não contém segredo e não é ignorado.


## Use sua própria ferramenta de IA em vez disso

Se você já trabalha no Claude Code, Cursor, Codex ou em outra ferramenta que fala MCP, você pode apontá-la para a scene aberta em vez de usar o painel integrado. MCP (Model Context Protocol) é uma forma padrão para uma ferramenta de IA chamar outro programa.

1. Abra o menu principal no logotipo do Creator Hub, selecione **Definições**e, na **AI** tab, expanda **Expose AI assistant MCP server**.
2. Clique em **Copy configuration**.
3. Cole isso na configuração MCP da sua ferramenta.

A configuração copiada fica assim, com uma porta e um token reais preenchidos:

```json
{
  "mcpServers": {
    "creator-hub": {
      "type": "http",
      "url": "http://127.0.0.1:<PORT>/mcp",
      "headers": { "Authorization": "Bearer <TOKEN>" }
    }
  }
}
```

Para o Claude Code, você também pode adicioná-lo a partir de um terminal na pasta da sua scene:

```bash
claude mcp add --transport http --scope local creator-hub http://127.0.0.1:<PORT>/mcp \\
  --header "Authorization: Bearer <TOKEN>"
```


> [!WARNING]
> **📔 Nota**: A porta e o token são novos toda vez que o Creator Hub é iniciado. Se sua ferramenta de repente receber um erro, copie a configuração novamente. O servidor só escuta na sua própria máquina e só atua na scene que você tem aberta. `401` error, copie a configuração novamente. O servidor só escuta na sua própria máquina e só atua na scene que você tem aberta.


## Páginas relacionadas

* [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/): o guia completo para as SDK Skills e para fazer prompts bem.
* [Combinar com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/): abra sua scene em um editor de código.
* [Usando o Script Component](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/): anexe comportamento a uma entidade a partir de um arquivo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
