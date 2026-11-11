> Crie scenes, wearables e emotes do Decentraland. O Creator Hub é a ferramenta recomendada, com SDK Skills para assistentes de codificação com IA e uma CLI para fluxos de trabalho avançados.

# Bem-vindo, Criador

Vamos construir Decentraland juntos!

Todos os criadores são bem-vindos! Em Decentraland, você tem uma ampla gama de possibilidades criativas, para pessoas com diferentes talentos e níveis de habilidade!

Decentraland está disponível para desktop (Windows e macOS) e para dispositivos móveis (iOS e Android). Os jogadores se movem sem interrupções entre clients com a mesma conta, avatar e inventário — o que significa que suas scenes precisam funcionar bem para todos, independentemente do dispositivo em que estejam jogando. Ao longo deste guia, você encontrará recomendações sobre como projetar, construir e testar suas scenes para que funcionem muito bem em todas as plataformas compatíveis. Para orientações específicas para mobile, veja [Construindo para mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/).

## O Creator Hub

O Creator Hub é a ferramenta recomendada para criadores de todos os níveis de conhecimento. É um aplicativo de desktop que permite criar [Scenes](#scenes).

![](/docs/_remote/32e1b985101d928b718678f9.webp)

Transferir o Creator Hub [aqui](https://decentraland.org/download/creator-hub).

## Wearables

Wearables são itens de vestuário que os avatars dos players podem usar. Eles são vendidos como NFTs e comprados no [Marketplace](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Saiba tudo sobre [Criando Wearables](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/).

Você também pode combinar um wearable com código do SDK para criar um [wearable inteligente](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables/). Isso ativa uma global scene sempre que o player veste o wearable. Veja [Tipos de projeto](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/) para compreender melhor as diferentes opções.

## Emotes

Emotes são animações que o avatar de um player pode fazer. Elas são vendidas como NFTs e compradas no [Marketplace](https://decentraland.org/marketplace/browse?assetType=item&section=emotes&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Saiba tudo sobre [Criando Emotes](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/creating-emotes/).

## Scenes

O conteúdo 3D em Decentraland é composto por scenes, cada scene ocupa uma quantidade finita de espaço e é exibida uma ao lado da outra para que os players possam caminhar livremente entre elas.

O Creator Hub permite que você crie scenes com uma interface simples de arrastar e soltar e também edite código para ter controle total sobre as interações. Você pode executar previews, fazer debug, editar código e publicar.

[Saiba mais](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/)

### Como eu crio uma scene em Decentraland?

1. **Instale o** [**Creator Hub**](https://decentraland.org/download/creator-hub), o aplicativo oficial de desktop para criar, fazer preview e publicar scenes. É a ferramenta recomendada para criadores de todos os níveis de habilidade.
2. **Se você usar um assistente de codificação com IA** (como Claude Code, Cursor ou Copilot), instale o oficial [Decentraland SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para que ele conheça os padrões verificados do SDK7: `npx skills add decentraland/sdk-skills`
3. **O** [**CLI**](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli/) é uma alternativa para usuários avançados e fluxos de trabalho automatizados.

### Arte 3D

As scenes de Decentraland são compostas por modelos 3D.

* Escolha entre o amplo catálogo de assets padrão no Scene Editor. Eles estão prontos para uso e otimizados para uso em Decentraland

  ![](/docs/_remote/b193f61ea826d9783815e90e.webp)
* Crie seus próprios modelos 3D usando Blender ou suas ferramentas 3D preferidas. Depois, importe-os para o Scene Editor.

  ![](/docs/_remote/6469c594dcf3e77e830e77da.webp)


> [!WARNING]
> **📔 Nota**: O conteúdo em Decentraland deve permanecer dentro de certos [limites de tamanho](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) para garantir que sua scene funcione sem problemas.
> 
> Veja [modelagem 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models/) para dicas e truques de otimização, e informações sobre recursos e formatos compatíveis para modelos 3D.


### Interatividade

Para tornar sua scene interativa:

* **Sem código**: Use a UI do Scene Editor para soltar [Smart Items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) na sua scene. Esses são modelos que vêm prontos com seu próprio comportamento e são altamente personalizáveis. Você também pode atribuir os mesmos comportamentos aos seus próprios modelos personalizados (sem necessidade de código).

  ![](/docs/_remote/a417642641d975327ba066bf.webp)
* **Code**: Para desenvolvedores que desejam incorporar lógica personalizada, use o SDK para escrever código e fazer qualquer coisa que você possa imaginar. Aprenda a usar o SDK:
  * [Início rápido do SDK](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/): siga este mini tutorial para um rápido curso intensivo.
  * [Fluxo de trabalho de desenvolvimento](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/dev-workflow/): leia isto para entender a criação de scenes de ponta a ponta.
  * [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/): crie scenes descrevendo o que você quer em linguagem simples e deixe um assistente de codificação com IA escrever o código para você.
  * [Exemplos](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/): mergulhe diretamente em scenes de exemplo funcionais.

    ![](/docs/_remote/3d5fae2810e08190c0c1ad38.webp)


> [!WARNING]
> **📔 Nota**: você também precisará ter [Visual Studio Code](https://code.visualstudio.com/) instalado.


### Publicando scenes

Você não precisa possuir nenhum token para começar a construir sua scene com o Scene Editor. Para publicar sua scene, você pode escolher entre as seguintes opções:

* **LAND em Genesis City**: Este é o principal open world em Decentraland, que é dividido em parcels de 16x16 metros. Compre um ou vários parcels adjacentes no [Marketplace](https://decentraland.org/marketplace/lands), e faça o deploy da sua scene lá.
* **Decentraland Worlds**: [aba Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) são seus próprios espaços no metaverso. Tudo o que você precisa é possuir um [nome de Decentraland](https://decentraland.org/marketplace/names/claim), e você pode publicar uma scene tão grande quanto quiser!

Veja [Tipos de projeto](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/) para compreender melhor as diferentes opções.

Veja [publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) para detalhes e opções especiais ao publicar uma scene, seja em Genesis City ou em Worlds.

## Recursos úteis

Confira [Recursos úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/) para uma lista selecionada de ferramentas, add-ons, bibliotecas de assets e projetos de exemplo que podem acelerar seu fluxo de trabalho de criação.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
