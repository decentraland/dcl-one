> Visualize e edite os dados ao vivo armazenados pelo Multiplayer Server da sua scene.

# Dados do Servidor

Cenas que usam um [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) podem persistir dados no servidor: tabelas de classificação, progresso do jogador, alterações no ambiente como portas abertas ou itens colocados, e muito mais. Estes dados são atualizados **em tempo real** à medida que os jogadores interagem com a sua cena publicada.

Pode ver todos estes dados numa UI web, e também pode **alterá-los diretamente a partir daí**. Quaisquer edições que fizer entram em vigor na cena em tempo real, sem necessidade de republicar. Isto inclui:

* **Dados da cena**: valores partilhados por todos os jogadores, como uma tabela de classificação ou o estado de uma porta.
* **Dados do jogador**: valores armazenados para um único jogador, como o seu progresso ou preferências.
* **Variáveis de ambiente**: valores de configuração e segredos que só o servidor pode ler.

## Aceda à UI de armazenamento

Pode abrir a UI de armazenamento diretamente em [decentraland.org/storage](https://decentraland.org/storage).

Também pode aceder a partir do Creator Hub:

1. Abra o **Gerir** secção do Creator Hub.
2. Clique nos **três pontos** ao lado de um local onde publicou conteúdo.
3. Selecione **Ver armazenamento**.

<img src="/docs/_remote/512f80176ec141582be43e70.webp" alt="opção Ver armazenamento no Creator Hub" width="400">

A UI de armazenamento mostra uma lista de todas as localizações de Worlds e LAND onde publicou cenas, ou onde tem permissões de operador. Abra uma cena para ver os seus dados, organizados em três separadores: **Cena**, **Jogador**, e **Ambiente**.


> [!NOTE]
> **💡 Dica**: Estes dados só existem para cenas que usam um Multiplayer Server e que armazenam dados através da `Storage` API. Veja [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) para saber como configurar isso.


## Dados da cena

A **Cena** O separador lista todas as variáveis armazenadas que são partilhadas por todos os jogadores, por exemplo uma tabela de classificação ou o estado atual do ambiente.

![Scene data tab](/docs/_remote/76bca9abaef9d73bc207163d.webp)

Os valores desta lista são atualizados em tempo real à medida que os jogadores interagem com a sua cena. Também pode editar ou remover qualquer uma destas variáveis clicando no ícone do lápis ou do caixote do lixo. As alterações são aplicadas pela cena em execução, por isso esta é uma forma prática de ajustar valores em tempo real, como reiniciar uma tabela de classificação, sem republicar.

## Dados do jogador

A **Jogador** O separador lista todos os jogadores que têm quaisquer dados armazenados no seu servidor. Pode procurar um jogador por endereço da wallet ou por nome e, em seguida, ver todos os dados associados.

Tal como acontece com os dados da cena, pode editar ou remover qualquer valor clicando no ícone do lápis ou do caixote do lixo.

Isto é especialmente útil para o suporte: se um determinado jogador relatar um problema na sua cena, pode procurá-lo e inspecionar os dados armazenados para compreender a sua situação. Se acabou preso num estado incorreto, por exemplo com dados contraditórios de uma versão anterior da sua cena, pode editar ou limpar os seus registos para o restaurar para um estado estável, sem voltar a fazer deploy de nada.

## Variáveis de ambiente

A **Ambiente** O separador lista todas as variáveis de ambiente definidas para a sua cena. Estes são valores de configuração que só o servidor pode ler, o que faz deles o local certo para **segredos** como chaves privadas ou códigos de resgate de recompensas: os valores nunca passam pela máquina de um jogador nem aparecem no código publicado da sua cena.

![Environment variables tab](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

A partir deste separador pode adicionar novas variáveis ou sobrescrever e eliminar as existentes usando o ícone do lápis ou do caixote do lixo. Note que não pode *ler* o valor atual de uma variável; isto é intencional, para proteger dados sensíveis. Só a pode substituir ou eliminar.

As variáveis de ambiente também são excelentes para feature flags ou parâmetros de jogo, como a duração de uma partida ou o número máximo de jogadores, que pode querer ajustar na cena em tempo real sem republicar.

## Saiba mais

Para saber como ler e escrever estes dados a partir do código da sua cena, incluindo boas práticas para alterar a estrutura dos dados armazenados ao longo do tempo, consulte [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/operar-ao-vivo/server-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
