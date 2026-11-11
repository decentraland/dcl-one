> Crie seus próprios itens personalizados para reutilizar em qualquer uma das suas scenes.

# Itens Personalizados

Defina um Item Personalizado para o reutilizar facilmente em qualquer uma das suas cenas, ou partilhe-o com outros criadores de cenas. Os Itens Personalizados podem consistir numa única entity, ou em tantas entities quantas quiser. Os Itens Personalizados podem ser variações de Smart Items existentes, ou totalmente personalizados, com os seus próprios modelos e funcionalidade ajustados.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/7cGLu8P7dso" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=7cGLu8P7dso>">Watch on YouTube</a></p>



## Como Definir um Item Personalizado

Clique com o botão direito numa entity no [Entity Tree](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#the-entity-tree), ou selecione várias entities e depois clique com o botão direito nelas. Selecione **Criar Item Personalizado**.

![](/docs/_remote/52a9b492b9f5042f38574e21.webp)

Na secção inferior do ecrã, será então pedido que atribua um nome ao seu novo Item Personalizado.

![](/docs/_remote/eb32f501e5b7c6a70d610e46.webp)

O item está agora listado na **Itens Personalizados** aba, na parte inferior do ecrã. Esta aba só é apresentada se existir pelo menos um Item Personalizado na sua área de trabalho.

![](/docs/_remote/a66e7b1f2ae4e2d83577aeb6.webp)

Ao definir um item personalizado, pode selecionar várias entities ao mesmo nível hierárquico, mas não entities de pais separados se não partilharem um ancestral comum. Quaisquer entities aninhadas são incluídas automaticamente como parte do item personalizado; não precisam de ser selecionadas.

As entities originais na sua cena não são afetadas pela ação de definir um Item Personalizado.


> [!WARNING]
> **📔 Nota**: Ao definir um Item Personalizado, faz uma captura do estado de cada component em todas as entities selecionadas (exceto a posição, rotação e escala da root entity). Isto inclui **Actions**, **Triggers**, **Multiplayer**, **Scripts**, **Visibility**, e qualquer outro component.
> 
> Quaisquer actions e triggers de Smart Item irão autorreferenciar a sua própria cópia. Por exemplo, se definir um Item Personalizado que inclua um elevador e botões, os botões de cada cópia do elevador controlarão a cópia do elevador a que pertencem, e não a cópia original do elevador.


### Usar Itens Personalizados

Basta arrastar o item da **Itens Personalizados** aba para a sua cena.

Depois de adicionado, é livre de alterar qualquer propriedade de um Item Personalizado; as alterações que fizer só afetam *essa cópia* do Item Personalizado. Também pode editar ou eliminar os itens originais a partir dos quais o Item Personalizado foi definido; isso não afetará cópias existentes ou futuras.

Repare que os Itens Personalizados são apresentados com um ícone diferente na Entity Tree. No topo do menu de propriedades do item à direita, verá também uma menção a partir de qual Item Personalizado foram criados.

Para eliminar a definição de um Item Personalizado, clique com o botão direito no item no **Itens Personalizados** menu e selecione **Eliminar**. Esta ação não afeta quaisquer cópias existentes do item nas suas cenas; os Itens Personalizados órfãos permanecem na sua cena inalterados. Eliminar uma definição de Item Personalizado remove-a apenas da aba Itens Personalizados.

Para renomear a definição de um Item Personalizado, clique com o botão direito na definição de Item Personalizado na **Itens Personalizados** aba e selecione **Renomear**.

Não pode modificar a definição de um Item Personalizado que já foi criado; tem de criar uma nova definição e eliminar a original.

### Gerar Itens Personalizados dinamicamente

Cada Item Personalizado é armazenado como um composite, um ficheiro que descreve uma árvore de entities e components. Isto significa que pode gerar uma nova cópia de um Item Personalizado enquanto a cena está em execução, em vez de colocar cada cópia manualmente.

Pode gerar um Item Personalizado de duas formas:

* **Sem código**: Use a **Spawn Entity** ação no Scene Editor. Veja [Sobre a geração de entities](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/#about-spawning-entities).
* **Com código**: Use `Composite.instance()` no SDK. Veja [Composites](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/composites/).

### Partilhar Itens Personalizados

Pode partilhar os seus itens personalizados com outros criadores, para que os possam usar nas suas próprias cenas.

Os Itens Personalizados são armazenados cada um numa pasta separada na sua máquina local. Abra essa pasta clicando no ícone da pasta <img src="/docs/_remote/ad0c0dd6dced1bedf2b986b9.webp" alt="Code" data-size="line"> no canto superior direito da aba Itens Personalizados.

Também pode localizar manualmente essa pasta na sua máquina:

* No Windows: *User/AppData/Roaming/creator-hub/Custom Items*
* No Mac: *Users/username/Library/Application Support/creator-hub/Custom Items*


> [!WARNING]
> **📔 Nota**: A *Library* pasta está oculta no Mac por predefinição. A forma mais fácil de lhe aceder é abrir Go > Go to Folder e escrever *application support/creator-hub*


Para partilhar com outra pessoa, basta abrir a pasta Itens Personalizados e copiar a pasta completa do item. Pode querer compactar a pasta em zip para facilitar a transferência. Esta pasta contém tudo o que é necessário para usar o seu Item Personalizado.

A pessoa que estiver a usar o seu Item Personalizado deverá então descompactar a pasta do item na sua própria pasta Itens Personalizados na sua máquina. Talvez precise de clicar no botão Atualizar <img src="/docs/_remote/29e0f054f9a96c7fb0c004cd.webp" alt="Refresh" data-size="line"> para que o item apareça na sua **Itens Personalizados** aba.

Quaisquer **Assets** usados pelo seu Item Personalizado também são armazenados na pasta do Item Personalizado. Isto inclui quaisquer modelos 3D, imagens, scripts, sons e vídeos referenciados pelo item.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/custom-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
