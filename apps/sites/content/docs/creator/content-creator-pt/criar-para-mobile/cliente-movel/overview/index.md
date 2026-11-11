> Visão geral do aplicativo mobile do Decentraland para iOS e Android, com links para download e uma checklist para deixar as scenes prontas para mobile.

# Visão geral

Decentraland está agora disponível em mobile. Os jogadores podem explorar suas scenes em dispositivos iOS e Android, além do client de desktop e da web. Como criador, você pode adaptar suas scenes para que fiquem ótimas e funcionem bem em dispositivos com toque.

![App mobile do Decentraland — Genesis City](/docs/_remote/a10f68e6dd27dfa895f53969.webp)



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/FBr2gye3qh8" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=FBr2gye3qh8>">Watch on YouTube</a></p>



## Obtenha o app mobile

* [Baixar para iOS (App Store)](https://apps.apple.com/app/decentraland/id6478403840?utm_source=docs&utm_medium=internal&utm_content=ios)
* [Baixar para Android (Google Play)](https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer&pcampaignid=web_share&utm_source=docs&utm_medium=internal&utm_content=android)

## Construindo para Mobile

### Referência

* [Scenes de exemplo](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/sample-scenes/) — scenes de código aberto criadas pela equipe da Decentraland e otimizadas para jogar em mobile.
* [Recursos ausentes](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/missing-features/) — recursos disponíveis no client de desktop que ainda não têm suporte no app mobile.
* [Requisitos de hardware](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/hardware-requirements/) — especificações mínimas e recomendadas de hardware para executar Decentraland em mobile.

### Desenvolver

* [Detectar a plataforma pelo código](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) — use `isMobile()` para ramificar a lógica e a UI da sua scene por plataforma.
* [Pré-visualize sua scene no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/) — faça a pré-visualização diretamente em um telefone via o Creator Hub ou o CLI.
* [Área segura para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/) — as margens de hardware do dispositivo (notch, home indicator) e as regiões ocupadas pelos próprios controles do client; escolha a área em que sua UI é posicionada com `screenInset`.
* [Boas práticas de UI para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/) — práticas recomendadas e práticas a evitar, recomendações de dimensionamento e limitações atuais.
* [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/) — mapeamentos de input amigáveis ao toque e quais `InputAction`s a evitar.
* [Otimizar o Desempenho](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/optimize-performance/) — limites de scene mobile, como pré-visualizá-los no Creator Hub e metas de desempenho.

### Publicar

* [Destaque-se no Discover mobile](/docs/creator/content-creator-pt/criar-para-mobile/publicar/get-featured/) — requisitos de envio para a seção Discover mobile.
* [Curadoria no iOS](/docs/creator/content-creator-pt/criar-para-mobile/publicar/ios-curation/) — como a curadoria de conteúdo do iOS funciona e como enviar sua scene ou World para revisão.

## Checklist rápido

Antes de publicar uma scene que deve funcionar bem no mobile, certifique-se de que:

* [ ] Você pré-visualizou a scene em um dispositivo real — veja [Pré-visualize sua scene no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/).
* [ ] Todos os elementos críticos da UI permanecem dentro da [área segura do mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/).
* [ ] Sua UI tem tamanho suficiente para toque — siga as [melhores práticas de UI](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/).
* [ ] As ações principais não estão vinculadas a `IA_SECONDARY` ou `IA_ACTION_3`–`IA_ACTION_6` (a `F` e `1`/`2`/`3`/`4` teclas), cujos botões ficam ocultos por padrão no mobile, a menos que sua scene os mostre — veja [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/).
* [ ] Da sua scene [Desempenho](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/optimize-performance/) está acima de 90% no Perfil de Gráficos Altos em um telefone intermediário — veja [Requisitos de hardware](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/hardware-requirements/) para dispositivos de referência.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
