# Modelagem 3D por medidas com Codex — experimental, v6

A versão v6 constrói peças e maquetes por medidas e permite alterações posteriores
nos objetos nomeados. Usa geometria facetada com operações booleanas, não um kernel
industrial de superfícies analíticas. As ferramentas são as mesmas na interface,
no CLI e no assistente Codex.

## Começar no Windows

1. Extraia todo o ZIP numa pasta nova e abra `ABRIR-CADCRAFT.cmd`.
2. Use **Arquivo > Novo projeto 3D**. Surge uma base de 100 × 60 × 10 mm.
   O novo projeto ocupa outra aba e preserva os documentos existentes.
3. Abra **Janela > Assistente Codex**, conecte e entre com sua conta, se necessário.
   Comece uma conversa nova para o documento atual.
4. Peça a construção com unidade, medidas e posições. O agente pode substituir a
   base do projeto 3D, criar formas e cortes e verificar o resultado.
5. Salve em **CAD3D** para conservar a construção editável. Use **Exportar STL 3D**
   ou **Exportar STEP facetado** para intercâmbio de geometria.

O ZIP inclui `Exemplos/flange.cad3d` e `Exemplos/office.cad3d`. Abra pelo menu Arquivo
ou arraste o projeto para o app. São exemplos sintéticos de geometria, sem validação
estrutural, cargas, normas ou especificações de materiais.

## Exemplos de pedidos

- “Crie em mm uma flange de 100 mm de diâmetro e 10 mm de espessura. Faça um furo
  central de 30 mm e quatro furos de 8 mm, nos eixos X/Y a 35 mm do centro. Os
  cortadores devem atravessar toda a espessura. Use 48 segmentos no corpo e 24
  nos furos menores. Verifique as medidas e enquadre a vista.”
- “Na flange, mude a espessura do corpo para 12 mm e aumente a altura de todos
  os cortadores para 14 mm, começando em Z=-1. Preserve os diâmetros.”
- “Crie uma maquete em metros de um escritório de 8 × 6 m, paredes de 0,15 m e
  pé-direito de 3 m. Deixe a frente e o teto abertos para visualizar. Coloque uma
  porta de 1 × 2,2 m na parede direita e quatro mesas de 1,5 × 0,7 m. Dê nomes
  aos objetos para eu poder alterá-los depois.”
- “Faça uma peça por revolução em torno de Z com perfil fechado
  [raio,altura]: [0,0], [20,0], [20,5], [10,5], [10,30], [0,30], em mm.”

Um PDF, planta ou imagem anexada pode orientar a construção; não vira geometria
exata automaticamente. Informe as cotas ou permita que o agente pergunte pelos
valores ausentes. Peça dimensões provisórias explicitamente quando for uma maquete.

## Desenho 2D para 3D

Selecione círculos ou polilinhas fechadas no plano XY e Z=0. Use **Ferramentas >
Extrudar seleção 2D**, informe altura e unidade. O Codex também pode chamar
`model.from2d` pelos handles da inspeção do desenho.

Cada contorno cria um volume independente. Contornos internos não são
interpretados automaticamente como furos: o agente deve subtraí-los do volume
externo. Desenhos abertos, imagens, texto, entidades em outro plano e contornos
que se cruzam são recusados. Curvas de polilinhas são aproximadas por segmentos.

A conversão retém os objetos 2D de origem, também no CAD3D salvo. Desfazer retorna
ao desenho anterior. `model.to2d` retorna aos objetos de origem; Desfazer recupera
a construção 3D. Exportar STL/STEP não conserva o desenho 2D nem a receita.

## Construção e edição

- Caixas: origem no canto inferior [0,0,0], dimensões X/Y/Z positivas.
- Cilindros/cones: eixo Z, base em Z=0, centro X/Y=0. Cone aceita raio superior.
- Esferas: centro na origem. Curvas têm 12–128 segmentos, padrão 48.
- Extrusão: perfil fechado simples, inclusive côncavo, altura positiva e até 16
  perfis de corte. Pode omitir a repetição do primeiro vértice no fim.
- Revolução: perfil fechado simples [raio,Z], raios não negativos; volta de 360°.
- União, subtração e interseção: operandos nomeados anteriormente na receita.
  Os operandos ficam ocultos; os resultados e demais objetos independentes aparecem.
- Cada objeto tem posição XYZ e rotação em graus: X, depois Y, depois Z em torno
  da origem local; depois aplica-se a posição. Transformações do modelo completo
  são aplicadas por último e preservadas em edições individuais.
- Desfazer/Refazer funciona para construção, edição, conversão e transformações.

No Codex, `model.build` constrói/substitui uma receita completa; `model.edit`
substitui campos de um objeto e reconstrói seus dependentes; `model.add/remove`
alteram a construção. `mesh.inspect` informa receita, transformação global, unidades,
limites e contagem de triângulos. `ui.mesh.view` controla apenas a câmera.
Use `cadcraft_commands` para os parâmetros completos. Não há código executável
no formato de projeto. Operações inválidas são recusadas antes de modificar o desenho.

Uma construção nova só pode ocupar desenho vazio ou projeto CAD3D existente.
Geometria 2D ou STL existente nunca é apagada por `model.build`. O usuário pode
criar outra aba pelo menu Novo projeto 3D. O agente conserva a proteção por documento:
depois de trocar de aba, comece uma nova conversa.

A seção **Objetos da construção** mostra os nomes e um editor avançado de campos
JSON do objeto selecionado. Para trabalho normal, peça as alterações ao Codex.
Vistas, movimento, rotação, escala e enquadramento continuam disponíveis.

## Arquivos e precisão

**CAD3D:** JSON versionado com receita editável, transformação do modelo, unidade
atual e desenho 2D original, quando houver. Até 16 MiB. A recuperação automática
conserva a receita. Importações STL não ganham receita retroativamente.

**STL:** geometria triangular em um ou mais volumes. Não conserva nomes, receita,
cores, materiais ou desenhos 2D. O cabeçalho CADCraft registra a unidade, mas outros
programas podem ignorá-la; confira a unidade ao importar.

**STEP/STP:** exportação AP203 `FACETED_BREP`, com superfícies planas trianguladas
e unidades normalizadas para mm. Cada objeto final da construção é exportado como
volume separado. Não é exportação de cilindros/esferas analíticos nem de histórico
paramétrico. Verifica cascas fechadas e orientação; recusa malhas abertas, faces
inconsistentes, cascas invertidas e cavidades internas isoladas que exigiriam
representação adicional. Importação STEP não está implementada.

Para um único volume físico, una as partes sobrepostas ou que se tocam com
booleanas. Objetos de maquetes permanecem volumes independentes; paredes e mesas
podem se tocar sem união. A verificação de malha fechada não detecta todas as
interferências, auto-interseções ou condições de fabricação.

As medidas de construção são numéricas, mas circunferências e superfícies curvas
são facetadas. A diferença radial máxima de um polígono inscrito é
`raio × (1 - cos(pi / segmentos))`: raio 50 mm / 48 segmentos ≈ 0,107 mm;
128 segmentos ≈ 0,015 mm. Aumentar segmentos custa processamento. Confira a
precisão exigida pela peça antes de escolher STL/STEP facetado.

Limites conservadores: 128 objetos, até 32 booleanas e 4096 vértices de perfil no total,
perfis individuais até 256 vértices, 250 mil polígonos
intermediários, 20 mil polígonos por par booleano, árvore BSP até 192 níveis e
orçamentos de trabalho/refinamento. Booleanas com esferas densas ou muitos cortes
podem exigir reduzir segmentos ou simplificar a construção. Erros são explícitos;
a geometria anterior é preservada. Recursos de filetes, roscas, restrições 3D,
chapas, cálculos de tensão, GD&T e sólidos analíticos não estão implementados.

## Validação desta versão

Testes cobrem volumes conhecidos, primitivas, extrusão côncava, revolução, rejeição
de perfis inválidos, booleanas, fechamento/orientação, flange com cinco furos,
maquete com porta, CAD3D/STL/STEP, alterações de medidas, transformações, fonte 2D,
recuperação, Undo/Redo, controles e ferramentas do agente com proteção por documento.
O workflow Windows executa os testes e um fluxo real de CLI antes de publicar.

A integração com o Codex usa o mesmo caminho de ferramentas testado localmente;
um pedido com conta ChatGPT conectada requer validação no computador do usuário.
A verificação completa `cargo xtask ci` tem bloqueios anteriores de Clippy em
`geom/nurbs.rs` e `xtask/layers.rs`; esses avisos não são desta funcionalidade.
