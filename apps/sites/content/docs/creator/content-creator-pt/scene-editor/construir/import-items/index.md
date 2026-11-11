> Importe seus próprios modelos 3D, imagens, sons etc. para usar nas suas scenes.

# Importar assets personalizados

Você pode importar seus próprios modelos 3D para o Scene Editor. Escolha modelos entre uma ampla seleção de fontes gratuitas ou pagas na internet, ou crie seus próprios modelos personalizados. Você também pode importar outros assets, como imagens, arquivos de som e vídeos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/UepXpH-k0EI" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=UepXpH-k0EI>">Watch on YouTube</a></p>



## Importar um asset

Para importar um modelo 3D, uma imagem, um arquivo de som ou um vídeo para sua cena a partir do seu disco local:

1. Arraste arquivos diretamente para o painel inferior. Você também pode clicar no **+ Import Assets** botão no canto superior esquerdo do painel inferior e selecione no seu disco local.

![](/docs/_remote/7a7e1bf757e28411bf4bd633.webp)

2. Verifique a miniatura do modelo e clique em **Importar**. Ao importar múltiplos assets, use os botões de seta para percorrer cada asset.

![](/docs/_remote/114b427e0cc71cef23245e2a.webp)

Agora você pode encontrar seu asset na **Local Assets** aba. Os assets são organizados em pastas por tipo: os modelos 3D aparecem na *assets/Models* pasta, imagens em *assets/Images*, arquivos de som em *assets/Audio*, e vídeos em *assets/Video*.

Os itens dos asset packs gratuitos integrados são armazenados em *assets/asset-packs/*, e itens personalizados criados no editor em *assets/custom/*. Cenas mais antigas podem ter importações do usuário em *assets/scene/* em vez disso. Todos esses paths funcionam no código da sua cena, basta referenciar a pasta em que o asset está.

* Para modelos 3D, arraste os `.glb` ou `.gltf` arquivos para o canvas para adicioná-los como itens na sua cena.
* Você também pode arrastar uma imagem, um som ou um arquivo de vídeo para o canvas. O editor cria um item pronto que já aponta para o seu arquivo:

  * Um `.png`, `.jpg` ou `.jpeg` arquivo se torna um **Imagem** item.
  * Um `.mp3`, `.ogg` ou `.wav` arquivo se torna um item de som ambiente.
  * Um `.mp4` arquivo se torna um **Tela de vídeo** item.

  O novo item recebe o nome do arquivo e vem com os mesmos components e settings do item equivalente dos asset packs, então você pode ajustá-lo imediatamente no inspector.
* Qualquer asset também pode ser arrastado para os fields de um item que já está na cena. Por exemplo, você pode arrastar um `.mp3` arquivo para o *Path* field de um *Audio Source* component.


> [!NOTE]
> **💡 Dica**: Você também pode colar arquivos diretamente na `assets` pasta do projeto, a partir do Finder, Explorer ou de qualquer outro lugar. Eles aparecem na **Local Assets** aba por conta própria, pouco depois de a cópia terminar. Se não aparecerem, pressione o **Atualizar** botão ao lado do **Import Assets** .
> 
> <img src="/docs/_remote/bef94fa811ae48e5d4c837f0.webp" alt="" data-size="original">


### Formatos suportados

#### Áudio

Os seguintes formatos de Audio são suportados:

* *.mp3*
* *.wav*
* *.ogg*

#### Imagem

Os seguintes formatos de imagem são suportados:

* *.png*
* *.jpg*
* *.jpeg*

#### Video

Os seguintes formatos de vídeo são suportados:

* *.mp4*

#### Modelos 3D

Os seguintes formatos de modelo 3D são suportados:

* *.glTF*
* *.glb*

Ambos podem incluir arquivos de imagem de textura externos, ou binários externos (*.bin*) files.

Você pode converter outros formatos para esses formatos com vários outros editores e ferramentas. Veja [modelagem 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models/) para recomendações e dicas.

Todos os materials nos modelos precisam ser ou *basic material* ou *PBR*, e todas as textures precisam estar em tamanhos que sejam potências de dois (ex.: 256, 512). Veja [Limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) para mais detalhes.

Cada arquivo importado, de qualquer tipo, deve ocupar menos de 50 MB para poder ser usado em uma cena. Arquivos maiores não são suportados.

**Bibliotecas gratuitas de modelos 3D**

Em vez de criar seus próprios modelos 3D, você também pode baixá-los de várias bibliotecas gratuitas ou pagas, ou gerá-los com ferramentas de IA. Veja [Recursos úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/) para uma lista de bibliotecas de assets recomendadas e ferramentas de IA generativa.


> [!WARNING]
> **📔 Nota**: Preste atenção às restrições de licença que o conteúdo baixado possui.


Observe que, em vários desses sites, você pode escolher em qual formato baixar o modelo. Escolha sempre *.glTF* ou *.glb* o formato, se disponível. Se não estiver disponível, você precisará convertê-los para *.glTF* ou *.glb* antes de poder usá-los em uma cena. Para isso, recomendamos importá-los para Blender e exportá-los com um dos export add-ons disponíveis. *.glTF* export add-ons.

### Colliders

Você pode perceber que, ao executar um preview, o player consegue atravessar seus modelos 3D importados. Isso provavelmente acontece porque os modelos não têm um *collider mesh* para definir uma geometria de colisão. Veja [colliders](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/) para mais detalhes e instruções.


> [!NOTE]
> **💡 Dica**: Em vez de editar o modelo para adicionar um *collider mesh*, uma alternativa mais simples é adicionar um *Parede invisível* smart item com aproximadamente o mesmo formato para substituí-lo.


### Animações

Se um modelo importado incluir animações, a primeira animação incorporada ao modelo será reproduzida em loop.

Observe que você não tem controle sobre quando a animação começa ou para, nem sobre qual delas é reproduzida no caso de várias animações.

Se houver vários players na cena, eles podem estar vendo a animação fora de sync entre si.

Para alterar esse comportamento, você pode incluir um **Animator** component. Veja [Tornar qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/) para ferramentas no-code para tornar seu item interativo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/construir/import-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
