> Otimize sua scene para carregar rapidamente e funcionar sem problemas para todos os jogadores.

# Otimização de Desempenho

Há vários aspetos que pode otimizar nas suas scenes para garantir a melhor experiência possível para os jogadores que as visitam. Este documento aborda algumas boas práticas que podem fazer uma grande diferença na rapidez com que a sua scene carrega e na suavidade com que funciona para os jogadores que nela estão ou em scenes vizinhas.

Tenha em conta que muitos jogadores podem estar a visitar Decentraland usando hardware que não foi concebido para jogos, através do browser, ou a partir do [app móvel](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) num telefone — tudo isso limita quanta capacidade de processamento está disponível para a sua scene. A experiência de visitar a sua scene deve ser fluida para todos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tc1PwYKW1Kc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tc1PwYKW1Kc>">Watch on YouTube</a></p>



Consulte [Recursos Úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/) para ferramentas que podem ajudar, como o Decentraland Scene Optimizer, que extrai, deduplica e comprime textures dos modelos 3D da sua scene.


> [!NOTE]
> **📱 Mobile**: Os dispositivos móveis são normalmente o client com maiores restrições de recursos. Se a sua scene tiver como alvo jogadores em mobile, veja também [Construindo para Mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) para orientação específica para mobile.


O Explorer do Decentraland aplica muitas otimizações ao nível do engine. Essas otimizações fazem uma grande diferença, mas o desafio de renderizar várias experiências geradas por utilizadores em simultâneo num browser é grande. Precisamos da sua ajuda para fazer as coisas funcionar com fluidez.

## Temporização

### Reprodução de vídeo

Reproduzir vídeos é uma das coisas mais dispendiosas para o engine processar. Se a sua scene incluir vídeos, certifique-se de que apenas *UM* há uma VideoTexture em utilização de cada vez. Pode ter dezenas de planos a partilhar a mesma VideoTexture sem impacto significativo no desempenho, mas assim que adiciona uma segunda VideoTexture, os seus efeitos no framerate tornam-se muito notórios.

Deve também evitar ter vídeos a ser reproduzidos em regiões onde não podem ser vistos. Por exemplo, se tiver um ecrã no interior, alterne o vídeo usando uma trigger area com base em quando o jogador entra e sai.


> [!NOTE]
> **💡 Dica**Um truque usado por várias scenes é fazer streaming de um único vídeo com várias regiões mapeadas de forma diferente para diferentes planos. Cada ecrã de vídeo usa [UV mapping](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#using-textures) para mostrar apenas uma parte distinta da VideoTexture. Graças a isso, pode parecer que há vídeos separados a ser reproduzidos sem o custo de várias VideoTextures.



> [!NOTE]
> **💡 Dica**: Quando os jogadores estão fora da sua scene, as VideoTextures não são atualizadas em every frame. Isto ajuda a reduzir o impacto nas scenes vizinhas. Mesmo assim, o ideal é só ativar a reprodução de quaisquer vídeos quando os jogadores [entram na sua scene](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-enters-or-leaves-scene) .


### Lazy loading

Se a sua scene for grande, ou tiver áreas interiores que nem sempre estão visíveis, pode optar por não carregar todo o conjunto de entities desde o início. Em vez disso, carregue o conteúdo por região à medida que o jogador visita diferentes partes da scene. Isto pode reduzir significativamente o tempo de carregamento da scene, bem como a quantidade de textures e conteúdo 3D que o engine precisa de processar em cada frame.

Por exemplo, o edifício principal de um museu poderia carregar desde o início, mas as pinturas em cada piso só seriam carregadas para cada jogador à medida que visita cada piso.

Veja [esta scene de exemplo](https://github.com/decentraland-scenes/lazy-loading) para perceber como isso pode funcionar.

Para obter o melhor resultado em termos de evitar soluços, esconda entities alterando a propriedade da sua shape `visible` para false. Com esta abordagem, adiciona-as ao engine quando as cria, mas simplesmente não torna os seus modelos visíveis.

Uma alternativa é não adicionar as entities ao engine até serem necessárias. Isto pode resultar em alguns soluços quando as entities aparecem pela primeira vez, e também podem demorar alguns segundos até ficarem visíveis. A vantagem desta abordagem é que é uma forma válida de contornar o [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/). Tenha em conta que a contagem de limitações da scene é para o conteúdo que está a ser renderizado na scene em qualquer momento, não para o conteúdo total que poderia ser renderizado. Carregar e descarregar partes da scene deve permitir contornar essas limitações.


> [!WARNING]
> **📔 Nota**: As entities que não estão visíveis mas são adicionadas ao engine contam para as limitações da scene.


Também pode ativar ou desativar animations para entities que estão longe ou ocultas. Por exemplo, para um NPC que reproduz uma idle animation muito subtil, podia fazer com que só reproduzisse essa animação quando o jogador estiver a menos de 20 metros. Use uma trigger area à volta do NPC e ative ou desative as suas animations em conformidade.


> [!NOTE]
> **💡 Dica**: Quando uma entity está longe e suficientemente pequena, é removida pelo engine através de culling. Este culling ajuda ao nível de drawcall; remover entities do engine é sempre melhor. Este culling também não tem em conta a oclusão por outras entities, por isso as entities que não são tão pequenas mas estão escondidas por uma parede continuam a ser renderizadas.


### Blocos assíncronos

Blocos de [código async](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/) não bloqueiam o progresso de tudo o resto enquanto aguardam: a scene continua a correr, e o bloco async retoma quando a resposta aguardada chega. Note que as scenes correm num único thread, por isso isto não é processamento paralelo; apenas evita que a scene fique bloqueada enquanto aguarda respostas externas.

Quaisquer processos que dependam de respostas de serviços assíncronos, como `getPlayerData()` ou `getRealm()` devem executar-se sempre em blocos async, caso contrário bloqueiam o resto do carregamento da scene enquanto aguardam uma resposta. O mesmo aplica-se a quaisquer chamadas para servidores de terceiros.

Note que a scene será considerada totalmente carregada quando tudo o que não for async estiver concluído. Os processos async podem ainda estar a decorrer quando o jogador entra na scene. Evite situações em que um processo async resulte no carregamento de uma entity que possa potencialmente prender o jogador dentro da sua geometry.

### Baseie-se em Events

Tente fazer com que a lógica da scene dependa de ouvir [events](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/) o máximo possível, em vez de executar verificações every frame.

A `update()` função num [system](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) executa-se em every frame, 30 vezes por segundo (idealmente). Evite fazer verificações recorrentes se, em vez disso, puder subscrever-se a um event.

Por exemplo, em vez de verificar constantemente os wearables do jogador, pode subscrever-se ao `onProfileChanged` event, e verificar os wearables do jogador apenas quando estes mudarem.

Se tiver de usar um System, evite fazer verificações ou ajustes em every single frame. Pode incluir um temporizador como parte da função update e executar a verificação apenas uma vez por cada segundo completo, ou no período que fizer sentido.

## Otimize os modelos 3D

Há várias formas de otimizar os seus modelos 3D para serem mais leves.

Ao trabalhar com o [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/), pode ver estatísticas sobre os recursos usados pelos modelos 3D na sua scene, e se ultrapassarem algum dos [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/).

![](/docs/_remote/ab87e37107598ee524831639.webp)

Pode expandir este menu para ver detalhes.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)

Aqui ficam algumas dicas para melhorar estes indicadores:

* Sempre que possível, partilhe textures entre modelos 3D. Uma boa prática é usar uma única texture como mapa atlas, partilhada por todos os modelos na scene. É melhor ter 1 texture partilhada grande de 1024x1024 píxeis do que várias pequenas.

  > Nota: Evite usar o mesmo ficheiro de imagem tanto para a albedo texture como para o normal map ou o emissive map de um Material. Use ficheiros separados, mesmo que sejam idênticos. Atribuir o mesmo ficheiro de imagem a diferentes tipos de propriedades de texture pode introduzir artefactos visuais indesejados quando comprimido para asset bundles.
* *.glb* é um formato comprimido, vai sempre pesar menos do que um *.gltf*. Por outro lado, com *.gltf* é fácil partilhar imagens de texture exportando textures como um ficheiro separado. Pode ter o melhor dos dois mundos usando o [seguinte pipeline](https://github.com/AnalyticalGraphicsInc/gltf-pipeline), que lhe permite ter *.glb* modelos com ficheiros de texture externos.
* Evite usar transparencies com blending. As transparencies com blending têm de contornar várias das otimizações de renderização. Se possível, prefira geometry opaca ou com alpha testing.
* Evite skinned meshes. Podem prejudicar significativamente o desempenho.


> [!NOTE]
> **💡 Dica**: Leia mais sobre boas práticas para modelos 3D na \[Secção de 3D Modeling]\(/creator/3d-modeling/3d-models


### Recupere um modelo pesado no Blender

Quando as estatísticas apontam para um modelo específico, é normalmente mais rápido corrigir o modelo do que contorná-lo no código. As edições mais úteis, no Blender ou em qualquer ferramenta semelhante:

* **Reduza a contagem de triangles** com o modificador Decimate. Esta é uma ferramenta de recuperação para modelos que já tem. Os novos modelos ficam melhor modelados em low-poly desde o início.
* **Redimensione textures** para uma potência de dois, 1024x1024 ou menor, e reorganize-as no *.glb*.
* **Apague faces que o jogador nunca vê**, como a parte de baixo e as costas de props. Ative [back-face culling](#backface-culling) em vez de duplicar geometry.
* **Junte materials** num só, usando uma única atlas texture em vez de uma texture por parte.
* **Remova lights, cameras e materials de `_collider` meshes.** O engine ignora-os, por isso são peso puro.

Verifique a contagem de triangles e a contagem de materials antes e depois, depois exporte novamente para o mesmo ficheiro. Se estiver um preview a correr, ele recarrega com as suas alterações.

### Reutilizar o mesmo modelo muitas vezes

As scenes estão muitas vezes cheias de conteúdo repetido: postes de iluminação ao longo de uma rua, cadeiras numa sala, árvores num parque. A melhor forma de construir isto é a mais simples — **dar a cada cópia a sua própria entity e apontá-las todas para o mesmo&#x20;*****.glb*****&#x20;arquivo.**

```ts
// BOM: um ficheiro, muitas entities
for (const position of lampPostPositions) {
  const lampPost = engine.addEntity()
  Transform.create(lampPost, { position })
  GltfContainer.create(lampPost, { src: 'assets/scene/lampPost.glb' })
}
```

O engine reconhece que estas entities partilham uma source. O ficheiro é descarregado uma vez, convertido para asset bundle uma vez, e os seus meshes e textures ficam guardados em memória uma vez — o vigésimo poste de iluminação quase não custa nada para além da sua própria posição no mundo. Isto também funciona entre scenes: se uma scene vizinha usar o mesmo ficheiro, ele já está em memória. Exportar vinte ficheiros quase idênticos *.glb* em vez disso significa vinte downloads e vinte cópias em memória.

O que isto **não** poupa são draw calls. Vinte postes de iluminação são vinte objetos para desenhar, quer venham de vinte entities quer de um único *.glb* que contém vinte postes de iluminação modelados no seu interior. A única forma de reduzir essa contagem é unir os meshes na sua ferramenta de modelação, o que envolve um verdadeiro compromisso — veja [Instancing vs Duplicating vs Merging](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/meshes/#instancing-objects-vs-duplicating-objects) para ver quando vale a pena.

Algumas dicas relacionadas:

* **Agrupe em clusters em vez de um modelo gigante.** Se tiver muitos props espalhados, um bom meio-termo é um *.glb* por grupo — um quarteirão de uma rua, o mobiliário de uma sala — em vez de um modelo por prop ou um modelo para toda a scene. Reduz o número de objetos, mantendo cada cluster suficientemente pequeno para que o culling continue a ser útil.
* **A criar muitas cópias de uma vez?** Pré-carregue primeiro o modelo com o [`AssetLoad` componente](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/pre-load-resources/). As cópias são então criadas a partir de um modelo que já está em memória, em vez de cada uma esperar pelo mesmo download. Criar cada cópia continua a exigir trabalho, por isso um grande pico pode custar-lhe um frame de qualquer forma — o pré-carregamento remove a espera pelo ficheiro, não o custo de criar as cópias.
* **Um modelo muito grande é mais pesado para o engine do que muitos pequenos.** O engine distribui o trabalho de construir a sua scene por vários frames para evitar soluços, mas não consegue dividir um único modelo enorme — isso cai num só frame. Muitas peças pequenas carregam de forma fluida; uma peça monolítica tem mais probabilidades de causar um soluço visível.
* **Repetir um modelo não introduz novas textures nem geometry.** Vinte postes de iluminação construídos a partir de um modelo partilham os meshes e textures desse modelo em vez de acrescentar ao conjunto — o vigésimo não custa nada em memória de textures. Materials são a exceção: o engine cria uma instância de material por objeto renderizado para aplicar o clipping de fronteira da sua scene, por isso a contagem de materials segue o número de objetos que renderiza, não o número de modelos que usa. Essas instâncias continuam a partilhar as mesmas textures e variantes de shader, por isso custam um pouco de memória e não tempo de frame. O que empurra uma scene para o limite de textures [limite](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) é ter muitos *diferentes* modelos, cada um com o seu próprio conjunto feito à medida — por isso consolidar a sua scene em torno de uma biblioteca mais pequena de modelos reutilizados ajuda nesse ponto.

### Backface Culling

Para otimização de desempenho, o Backface Culling será definido para **On** em **todos os** nos Materials do modelo quando renderizado no engine, independentemente das suas definições.

Se espera ver a face traseira ou o interior dos seus modelos, duplique as faces e inverta as normals.

#### Resolução de problemas

Para verificar se uma scene tem problemas de Backface Culling do material, siga estes passos:

1. Abra o `debug` panel na scene.

* Se a scene estiver publicada, escreva `/debug` comando no chat.
* Se estiver no modo Preview na scene, clique no ícone de Bug (

  <img src="/docs/_remote/3a2d6a1437dc36383f2065db.webp" alt="ícone de debug" width="32">

  ) localizado no canto superior direito do ecrã.

2. O debug panel aparecerá no canto inferior direito do ecrã.
3. Em **Current Scene**, clique no **Backface debugger** button.

![](/docs/_remote/1e0c76604bd561268516ee00.webp)

4. Alternar **Force Backface Culling**: Mostra os Materials renderizados com Backface Culling On. Esta é a renderização real quando a Optimization estiver em produção. Alterne On e Off para identificar Materials que precisam de correção.
5. Alterne o **Backface debugger** para identificar facilmente Materials que têm Backface Culling Off. Destaca:

* **Vermelho**: Materials que não têm Backface Culling definido para **On**.
* **Verde**: Materials com Backface Culling **On**.

<p align="center"><img src="/docs/_remote/0c934b5889f4f369d893365d.webp" alt=""> <img src="/docs/_remote/d2530a38414758e20fb9ebd0.webp" alt=""><br><em>Material com Backface Culling Off (à esquerda) vs. Backface Culling On (à direita)</em></p>

### Conversão para Asset Bundle

Sempre que publica uma scene, os servidores de conteúdo do Decentraland comprimem cada *.gltf* e *.glb* modelo nela para formato asset bundle. Este formato é *significativamente* mais leve, tornando as scenes muito mais rápidas a carregar e mais fluidas a correr. A conversão começa imediatamente após cada deployment. Normalmente demora apenas alguns segundos, embora possa demorar mais em scenes muito grandes ou quando os servidores estão ocupados, uma vez que a conversão é executada separadamente para cada plataforma e pode ficar na fila atrás de outras scenes. Pode [verificar o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) da sua scene diretamente.

Também pode executar esta mesma conversão localmente enquanto desenvolve a sua scene. Isto torna o seu preview mais fluido e permite-lhe verificar quaisquer problemas com os modelos comprimidos antes de publicar. Veja [Pré-visualização com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 Dica**: Ao planear um evento no Decentraland, publique a versão final com pelo menos uma hora de antecedência, para que os modelos já estejam todos convertidos para asset bundles até lá e tenha tempo de corrigir qualquer coisa inesperada. Se não quiser estragar a surpresa antes do evento, pode fazer deploy de uma versão da sua scene que inclua todos os modelos 3D finais na pasta do projeto, mas em que estes não estejam visíveis ou em que o seu tamanho esteja definido para 0.



> [!WARNING]
> **📔 Nota**: Se fizer *qualquer* uma alteração a um ficheiro de modelo 3D, mesmo que seja apenas uma alteração de nome, ele será considerado um novo ficheiro e terá de ser novamente convertido para formato asset bundle.


## Conectividade

Se a sua scene se ligar a quaisquer servidores de terceiros ou usar o [messagebus](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/#send-explicit-messagebus-messages) para enviar mensagens entre jogadores, há também algumas coisas que pode querer ter em conta.

* A sua scene deve ter apenas uma ligação WebSockets ativa de cada vez.
* As chamadas HTTP são encaminhadas pelo engine para que apenas uma seja tratada de cada vez. Quaisquer pedidos adicionais são colocados em fila internamente e têm de aguardar até que outros pedidos sejam concluídos. Este processo de colocação em fila é tratado automaticamente; não precisa de fazer nada.
* Ao usar o [messagebus](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/#send-explicit-messagebus-messages) para enviar mensagens entre jogadores, tenha em atenção que todas as mensagens são enviadas para todos os outros jogadores na server island. Evite situações em que uma mensagem recebida resulte diretamente no envio de outra mensagem, uma vez que o número de mensagens pode crescer rapidamente de forma exponencial quando há multidão na scene.

## UI da Scene

As UIs da Scene podem tornar-se dispendiosas de renderizar quando são compostas por muitos elementos individuais. Tenha em conta que cada UI element requer um drawcall separado no engine.


> [!NOTE]
> **💡 Dica**: Tente fundir vários elementos numa única imagem. Por exemplo, se tiver um menu com vários elementos de texto, o ideal é que o texto dos tiles e quaisquer imagens adicionais sejam incorporados na imagem de fundo. Isso poupa ao engine fazer um drawcall adicional por frame para cada elemento de texto.


Evite fazer ajustes na UI em every frame; esses são especialmente dispendiosos e podem acabar por ficar em fila. Por exemplo, se houver uma barra de vida na sua UI que deva encolher ao longo do tempo, os jogadores provavelmente não notariam diferença se ela atualizasse a 10 FPS em vez de 30 FPS (em every frame). O System que atualiza esta barra pode usar um temporizador breve que conta 100 milissegundos, e só afetar a UI quando esse temporizador chegar a 0.

Evite ter muitos elementos de UI escondidos; estes também têm impacto no desempenho mesmo que não estejam a ser renderizados. Sempre que possível, tente criar components de UI sob demanda.

## Terreno de landscape em Worlds

Scenes publicadas em um [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) estão rodeados por uma landscape auto-gerada de relva, árvores e mar. Renderizar esta landscape consome parte do orçamento de renderização do jogador. Se a sua scene não precisar disso, pode [desativar o landscape terrain](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#landscape-terrain) na sua `scene.json` para libertar esses recursos para o conteúdo da sua própria scene.

## Monitorizar o desempenho

A melhor métrica para saber quão bem uma scene está a desempenhar-se é o FPS (Frames Per Second). Em preview, pode ver o FPS atual da scene no debug panel. Deve procurar ter sempre 30 FPS ou mais.

Na scene em produção, pode alternar o panel que mostra estas métricas escrevendo `/showfps` na janela de chat.

### Painel Scene Stats

Ao executar uma scene em preview no client de desktop, pode abrir um panel que mostra estatísticas em tempo real sobre o conteúdo da sua scene. Clique no ícone de estatísticas no canto superior direito do ecrã, junto aos ícones da console e debug.

<img src="/docs/_remote/96e07557afec5b2ade6528d5.webp" alt="Painel Scene Stats" width="400">

O panel conta os triangles, entities, bodies e textures da scene em relação aos respetivos limites, mostrando que percentagem de cada orçamento está em uso. Um valor fica laranja quando atinge 80% do seu limite. Também mostra contagens simples de materials, geometries, colliders e vídeos ou áudios externos. Passe o rato sobre o ícone de informação junto de cada métrica para obter uma explicação do que conta e por que razão isso importa.

Os números atualizam-se em tempo real à medida que o conteúdo carrega e descarrega, o que torna o panel útil para verificar como estratégias como [lazy loading](#lazy-loading) afetam o que o engine está realmente a processar em cada momento. Veja [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) para detalhes sobre cada limite. Tenha em conta que estes são limites suaves: exceder um deles não bloqueará a sua scene, mas pode degradar o desempenho para os jogadores.

### Mensagens entre a scene e o engine

Um dos principais gargalos no desempenho de uma scene é normalmente o envio de mensagens entre o código da scene e o engine.

Quando executa uma scene em preview, note que no canto superior direito aparece “Y = Toggle Panel”. Prima Y no teclado para abrir um panel com alguma informação útil que é atualizada em tempo real.

À medida que interage com coisas que envolvem mensagens entre o SDK e o engine, notará que o número de ‘Processed Messages’ aumenta. Deve observar de perto o número de ‘Pending on Queue’; deve estar sempre em 0 ou perto de 0. Isto diz-lhe quantas destas mensagens não chegaram a ser processadas e foram empurradas para uma fila. Se a contagem de ‘Pending on Queue’ começar a crescer, então entrou na zona de perigo e deve pensar em fazer mais otimizações à sua scene.


> [!WARNING]
> **📔 Nota**: Não mantenha o panel aberto enquanto não estiver a usá-lo, uma vez que isso tem impacto negativo no desempenho.


Tenha em conta que o desempenho que experiencia em preview pode diferir do da produção:

* As scenes vizinhas circundantes podem ter um impacto negativo
* A compressão dos modelos 3D das scenes para asset bundles pode ter um impacto positivo
* Alguns jogadores que visitam a sua scene podem estar a usar hardware menos potente

É sempre uma boa prática tentar fazer deploy da sua scene primeiro para um [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) para fazer testes mais aprofundados.

Peça sempre feedback aos jogadores. Nunca tome como garantido que a forma como experiencia a scene é a mesma para todos os outros.

## Otimize com IA

Um agente de IA pode tratar de grande parte da medição e correção descritas nesta página por si. O Decentraland disponibiliza [AI skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/#install-skills-for-any-ai-agent) que ensinam agentes de programação (como Claude Code ou Cursor) a otimizar scenes, e o client de desktop inclui um [MCP server](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/#let-the-ai-see-your-scene-in-world) que permite ao agente inspecionar e medir diretamente a sua scene em execução.

Com os `optimize-scene` e `unity-explorer-mcp` skills instaladas e o servidor MCP ligado, um agente pode:

* **Verificar a scene em relação aos seus content budgets**: lê as mesmas estatísticas em tempo real que o painel Scene Stats e compara-as com os [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/), para poder dizer-lhe qual orçamento está a ficar apertado.
* **Encontrar os assets responsáveis**: classifica os modelos 3D da sua scene pelo número de triangles, materials e draw calls que cada um contribui, incluindo o que é visível de um ponto de vista específico, para saber exatamente quais os modelos a otimizar primeiro.
* **Medir a taxa de frames real**: posiciona o jogador em qualquer ponto da scene e mede o FPS que o jogador realmente experiencia ali, incluindo soluços momentâneos.
* **Verificar o seu próprio trabalho**: após cada alteração, pode voltar a medir, para que as suas recomendações e correções sejam sustentadas por números reais em vez de suposições.

Isto transforma a otimização em algo que pode simplesmente pedir:

> "Verifique se a minha scene está dentro dos limites, encontre o que quer que esteja a reduzir a taxa de frames perto do spawn point, corrija-o e mostre-me os números antes e depois."

### Combine com o Blender MCP server

As ferramentas do Decentraland dizem ao agente quais os modelos mais pesados, mas não editam os próprios modelos. Para ir mais longe, pode tentar combiná-las com o [Blender MCP server](https://www.blender.org/lab/mcp-server/), que dá ao agente acesso às ferramentas de edição do Blender. Isto permite ao agente abrir um modelo pesado no Blender, reduzir a sua contagem de triangles, fundir os seus Materials ou reduzir as suas textures, e exportar o resultado de volta para a scene como um `.glb` ficheiro, onde depois pode voltar a medir a taxa de frames no mundo.

Tenha em conta que edições automatizadas de mesh podem alterar visivelmente o aspeto de um modelo, por isso reveja sempre os resultados e mantenha cópias de segurança dos seus ficheiros originais.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
