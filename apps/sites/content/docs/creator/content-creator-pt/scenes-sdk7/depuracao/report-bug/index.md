> Como você pode reportar um bug para que ele seja revisado e corrigido

# Reportar um Bug

Se encontrar quaisquer bugs ou problemas com o Decentraland SDK e o Scene Editor no Creator Hub, por favor reporte-os no nosso repositório do GitHub [aqui](https://github.com/decentraland/sdk/issues/new/choose).

## Antes de reportar um bug

Antes de enviar um relatório de bug, certifique-se do seguinte:

* Certifique-se de que está a usar a versão mais recente do SDK, caso o problema já tenha sido corrigido em lançamentos mais recentes.
* Consulte a documentação da funcionalidade que está a tentar usar, para confirmar que ela deve comportar-se como espera e que isto não se trata apenas de um mal-entendido.
* Exclua a possibilidade de o problema ser causado pela forma como a sua scene foi construída. Consulte [Depurar no Preview](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-preview/) e [Depurar em Prod](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-prod/) para dicas.
* Evite duplicados. Consulte a [lista de bugs conhecidos](https://github.com/orgs/decentraland/projects/20/views/13) para evitar criar uma duplicada. Se tiver mais informação para adicionar a uma issue existente, adicione um comentário à issue em vez de criar uma nova issue.

## Reportar um bug

Para reportar um bug relacionado com o SDK, vai precisar de uma conta GitHub. Siga estes passos:

1. Visite o seguinte link: [Página de Nova Issue](https://github.com/decentraland/sdk/issues/new/choose)
2. Clique em **Começar** ao lado de **Relatório de Bug**. ![](/docs/_remote/b51c88e272f5593a42575857.webp)
3. Preencha todos os campos do template. O template fornece instruções para cada secção. Quanto mais detalhes puder fornecer, mais rapidamente os nossos developers poderão identificar e resolver a issue.
4. Clique em **Submeter nova issue**.

![](/docs/_remote/dc32a849c6f1a59a64d4565b.webp)

Os developers podem fazer perguntas de seguimento no seu ticket se precisarem de esclarecimentos ou informações adicionais. Receberá notificações por email sobre atualizações ao seu relatório de bug.


> [!WARNING]
> **📔 Nota**: Para quaisquer issues não relacionadas com o SDK, por favor contacte a equipa de suporte. Para entrar em contacto, visite esta página [intercom.decentraland.org](https://intercom.decentraland.org/) ou envie-nos um email para <hello@decentraland.org>. Também nos pode encontrar no [Discord](https://decentraland.org/discord/). A equipa está disponível para ajudar de segunda a domingo, das 12h às 21h UTC, e fará o possível para entrar em contacto novamente o mais rapidamente possível. Pode esperar uma resposta em 60 min no Discord durante o período acima e dentro de 24h por email.
> 
> Também pode contactar a equipa de suporte DAO no [DAO Discord](https://discord.gg/bxHtcMxUs4).


**Dicas adicionais para relatórios de bug:**

* Crie uma issue separada para cada bug. Se encontrar vários problemas, certifique-se de criar uma nova issue para cada um.
* Evite explicações longas. Inclua apenas saída relevante da linha de comandos ou da consola, ou código da scene, quando necessário.
* Anexe imagens ou vídeos, se possível. Pode facilmente arrastá-los e largá-los na área de texto da issue, e o GitHub tratará do carregamento e da ligação.

## Utilizar o Decentraland Playground

Fornecer trechos de código que demonstrem o problema é extremamente útil. A reprodutibilidade é crucial. Se um bug não puder ser reproduzido, é difícil diagnosticar e corrigir.

Pode colar trechos de código no texto da sua issue, mas a melhor forma de fornecer trechos de código é usando o [Decentraland Playground](https://playground.decentraland.org/). Siga estes passos:

1. Escreva o seu trecho de código à esquerda e veja a sua scene no painel da direita.
2. Clica no **Partilhar** botão e cole o link no seu relatório da issue.

![](/docs/_remote/14f730a1e519a0ed272964d1.webp)

Usar o playground simplifica a reprodução da issue e garante que qualquer pessoa que abra o link possa ver a mesma saída, independentemente do seu sistema operativo, versão do SDK, versão do Node, etc. Também facilita iterações rápidas para identificar as condições afetadas e localizar a causa da issue.

Mantenha o seu exemplo tão conciso quanto possível, ao mesmo tempo que reproduz a issue. Um exemplo mais simples ajuda a eliminar ambiguidade sobre a origem do problema.


> [!WARNING]
> **📔 Nota**: Não é possível importar assets para a sua scene no Playground. Portanto, se reproduzir a issue exigir a importação de um modelo 3D ou de um ficheiro de som, não será possível mostrá-lo aí.


## Obter logs

Muitas vezes é útil obter os logs do Decentraland Explorer ao reportar um bug.

Para obter os seus logs, basta abrir o chat e escrever `/logs`. Isto abrirá uma nova janela com os logs do Decentraland Explorer. Anexe o `Player.log` ficheiro ao seu relatório de issue.

## Editar a documentação

Se descobrir issues no conteúdo do site da Documentação, pode enviar pedidos de alteração. Basta deslocar-se até ao final da página e clicar no **Editar esta página** botão. O GitHub irá guiá-lo na criação de um pull request com as suas alterações. A equipa da foundation irá rever as suas alterações e publicá-las se forem consideradas úteis.

## Pedidos de funcionalidades e sugestões

Se tiver sugestões para adicionar nova funcionalidade ou funcionalidades em vez de reportar um problema, por favor junte-se ao [servidor Discord da Decentraland](https://dcl.gg/discord) e publique uma mensagem no [canal do Creator Hub](https://discord.com/channels/417796904760639509/1288888172318560326) em vez de criar uma Issue do GitHub.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
