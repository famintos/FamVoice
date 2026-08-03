# FamVoice — plano de correção e evolução por fases

> Estado da auditoria: 2026-08-02  
> Base analisada: `v0.3.29`, commit local `e6c9f53`  
> Objetivo: corrigir primeiro as falhas de integridade e fiabilidade, consolidar os testes e só depois modernizar a transcrição e adicionar funcionalidades.

## Como usar este documento

Este ficheiro é a fonte de execução do trabalho identificado na auditoria técnica da FamVoice. Cada fase tem tarefas, critérios de aceitação e um gate de saída. Uma fase só fica concluída quando:

- todas as caixas obrigatórias estão concluídas;
- os testes automatizados da fase passam;
- a prova nativa exigida está registada separadamente;
- não foram incluídas alterações fora do âmbito;
- o estado do repositório foi novamente inspecionado.

As caixas devem ser atualizadas durante a execução. Quando uma tarefa não for aplicável, marcar como `N/A` e explicar porquê no registo da fase.

## Regras permanentes

- Preservar alterações locais e trabalho alheio; não usar reset destrutivo nem formatação global.
- Não terminar à força uma instância FamVoice que possa estar em utilização.
- Não registar transcrições, áudio, API keys ou corpos de respostas remotas.
- Manter o widget compacto e a recuperação por tray/hotkey; não redesenhar o widget como efeito secundário.
- Manter o Groq funcional enquanto se moderniza o caminho OpenAI.
- Não atualizar dependências em massa. Separar patches seguros de alterações major, como TypeScript 7.
- Tratar testes automatizados, smoke nativo, estado remoto e release publicada como provas diferentes.
- Não criar tag, release, commit ou push sem autorização explícita para essa etapa.

## Estado inicial a confirmar

Snapshot observado na auditoria:

- `master` estava um commit atrás de `origin/master`;
- existiam 16 ficheiros modificados e 2 ficheiros novos no worktree;
- as alterações locais e o commit remoto sobrepunham-se em CI/release;
- `npm test`, build, lint, Rust tests, `cargo fmt`, Clippy e `git diff --check` passavam;
- `npm audit` falhava com uma vulnerabilidade high em `brace-expansion@5.0.7`, pela cadeia de desenvolvimento ESLint → minimatch;
- o GitHub apresentava um alerta Dependabot medium para `serde_with 3.18.0`;
- o CodeQL não tinha alertas abertos;
- a release e metadata do updater apontavam corretamente para `0.3.29`.

Este snapshot pode ficar desatualizado. Revalidar tudo no início da Fase 0.

---

## Fase 0 — consolidar a base de trabalho

### Objetivo

Integrar o estado local e remoto sem perder alterações, antes de corrigir comportamento da app.

### Tarefas

- [x] Inspecionar `git status --short --branch`, os diffs locais e o commit em falta no remoto.
- [x] Classificar os 18 caminhos já alterados por responsabilidade: segurança, dependências, workflows, backend e alterações não relacionadas.
- [x] Confirmar se os ficheiros locais de hardening formam um conjunto coerente e se pertencem a esta entrega.
- [x] Integrar cuidadosamente o commit remoto `4740bf8` ou o seu sucessor atual, resolvendo a sobreposição em CI/release sem apagar trabalho local.
- [x] Confirmar a paridade de versão entre `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` e `src-tauri/tauri.conf.json`.
- [x] Reexecutar os gates base listados na secção “Validação automatizada comum”.
- [x] Registar o novo commit base e o estado do worktree no “Registo de execução”.

### Classificação do worktree preexistente

- Segurança, governance e workflows (5): `.github/SECURITY.md`, `.github/workflows/ci.yml`, `.github/workflows/codeql.yml`, `.github/workflows/dependency-review.yml` e `.github/workflows/release.yml`.
- Dependências e tooling (4): `package.json`, `package-lock.json`, `src-tauri/Cargo.lock` e `vite.config.ts`.
- Superfície de capabilities Tauri (3): `src-tauri/capabilities/default.json`, `src-tauri/capabilities/default.test.mjs` e `src-tauri/capabilities/settings.json`.
- Hardening do backend (4): `src-tauri/src/history.rs`, `src-tauri/src/prompt_optimizer/mod.rs`, `src-tauri/src/settings.rs` e `src-tauri/src/transcription.rs`.
- Backend misto (1): `src-tauri/src/lib.rs` contém hardening de logs/contagem de caracteres e parte da correção funcional de auto-paste sem clipboard.
- Alteração funcional não relacionada com hardening (1): `src-tauri/src/injection.rs` implementa inserção direta de texto quando a cópia para clipboard está desligada.

O conjunto de hardening é coerente e pertence à entrega de segurança/qualidade já iniciada. A correção funcional de auto-paste é separada da Fase 0; foi preservada e identificada explicitamente, sem a ampliar nesta fase.

### Gate de saída

- O commit remoto relevante está integrado ou existe uma justificação explícita para o adiar.
- Nenhuma alteração local foi perdida.
- O âmbito das alterações preexistentes está documentado.
- O único gate que pode continuar vermelho é uma vulnerabilidade já catalogada para correção na Fase 4.

---

## Fase 1 — integridade do ciclo de ditado

### Objetivo

Garantir que cada gravação produz no máximo uma entrega coerente e que gravações consecutivas não interferem entre si.

### 1.1 Coordenador de sessões de transcrição — P1

Problema: uma transcrição antiga pode terminar depois de uma gravação nova e alterar o estado, histórico ou aplicação ativa fora de ordem.

Pontos atuais: `src-tauri/src/audio.rs:855`, `src-tauri/src/lib.rs:792` e `src-tauri/src/lib.rs:861`.

- [x] Introduzir um ID monotónico por sessão de gravação/transcrição.
- [x] Associar captura, transcrição, eventos de estado e entrega final ao mesmo ID.
- [x] Impedir uma sessão antiga de emitir `success`, `error` ou `transcript` sobre uma sessão mais recente.
- [x] Serializar a inserção final ou definir uma política explícita de fila/cancelamento.
- [x] Garantir que uma sessão ultrapassada continua recuperável no histórico quando apropriado, sem auto-colar inesperadamente.
- [x] Adicionar teste em que a primeira API fica bloqueada, começa uma segunda gravação e as respostas terminam fora de ordem.

### 1.2 Rejeitar respostas vazias — P1

Problema: uma resposta HTTP bem-sucedida mas vazia pode limpar o clipboard, criar histórico vazio e emitir sucesso.

Pontos atuais: `src-tauri/src/transcription.rs:218`, `src-tauri/src/transcription.rs:251` e `src-tauri/src/lib.rs:803`.

- [x] Validar o texto final depois de `trim()` e antes de clipboard, injeção ou histórico.
- [x] Converter resposta vazia num erro recuperável e compreensível.
- [x] Cobrir respostas vazias nos caminhos normal e streaming.
- [x] Confirmar que o clipboard e o histórico permanecem intactos.

### 1.3 Transações de re-paste e clipboard — P1

Problema: re-pastes rápidos partilham um único `saved_text`; restaurações atrasadas podem perder o clipboard original.

Pontos atuais: `src-tauri/src/clipboard.rs:4` e `src-tauri/src/lib.rs:227`.

- [x] Fazer cada operação transportar o seu snapshot imutável do clipboard ou serializar a transação completa.
- [x] Impedir que uma restauração antiga sobrescreva uma operação mais recente.
- [x] Testar duas e três operações sobrepostas com conteúdo Unicode e multilinha.
- [x] Confirmar o comportamento quando a leitura ou escrita do clipboard falha.

### 1.4 Falhas do stream de áudio — P2

Problema: um erro CPAL pode colocar `is_recording=false` sem emitir erro para o frontend, deixando o widget em “Listening”.

Pontos atuais: `src-tauri/src/audio.rs:628` e `src-tauri/src/lib.rs:135`.

- [x] Separar o estado “hotkey pressionada” do estado “stream saudável”.
- [x] Encaminhar uma falha de stream para a orquestração principal.
- [x] Emitir erro e reset de estado uma única vez.
- [x] Permitir uma gravação nova depois da recuperação.
- [x] Adicionar teste com erro/desconexão simulada do dispositivo.

### 1.5 Troca de microfone durante gravação — P2

Problema: guardar settings pode reconfigurar e descartar o stream ativo.

Pontos atuais: `src-tauri/src/lib.rs:330` e `src-tauri/src/audio.rs:749`.

- [x] Desativar/rejeitar a troca enquanto existe gravação ativa, ou adiá-la para o próximo `Start`.
- [x] Explicar o estado na UI de Settings.
- [x] Cobrir a troca de dispositivo durante gravação e durante transcrição.

### Gate de saída

- Cada sessão tem identidade própria.
- Respostas antigas, vazias ou falhadas não produzem efeitos laterais incorretos.
- Testes unitários e de integração de concorrência passam.
- Smoke nativo confirma dois ditados rápidos, uma falha de microfone e re-pastes rápidos.

---

## Fase 2 — persistência, segredos e recuperação

### Objetivo

Eliminar corrupção parcial e garantir que falhas de disco/keyring não são reportadas como sucesso.

### 2.1 Escrita atómica de histórico e settings — P2

Problema: os ficheiros são truncados e escritos diretamente; uma interrupção pode deixar JSON corrompido. Snapshots concorrentes também podem chegar ao disco fora de ordem.

Pontos atuais: `src-tauri/src/history.rs:81`, `src-tauri/src/history.rs:135`, `src-tauri/src/history.rs:140` e `src-tauri/src/settings.rs:684`.

- [x] Criar um helper partilhado de escrita atómica no mesmo diretório.
- [x] Escrever num ficheiro temporário com permissões adequadas.
- [x] Fazer flush/sync quando suportado e substituir o ficheiro final atomicamente.
- [x] Serializar/versionar escritas para impedir snapshots antigos de vencerem.
- [x] Manter uma última versão conhecida como boa sem sobrescrever repetidamente a única cópia de recuperação.
- [x] Testar disco cheio, falha a meio da escrita, ficheiro existente e operações concorrentes.
- [x] Confirmar que a migração de histórico plaintext para envelope DPAPI também usa o caminho atómico.

### 2.2 Falhas parciais do keyring — P2

Problema: uma falha do secure store pode ficar apenas no log e a UI pode tratar o save como concluído.

- [x] Tornar o backend de credenciais injetável em testes.
- [x] Cobrir keyring indisponível, escrita parcial e valor antigo no keyring.
- [x] Definir a autoridade entre keyring e fallback cifrado em disco.
- [x] Devolver um estado de recuperação sanitizado à UI, sem segredos.
- [x] Garantir que nenhuma API key volta a ser persistida em claro.

### 2.3 Limites da inserção direta — P3

Problema local: quando a cópia para clipboard está desligada, a inserção direta aceita texto potencialmente muito grande.

Pontos atuais: `src-tauri/src/lib.rs:812`, `src-tauri/src/injection.rs:92` e o limite de re-paste em `src-tauri/src/lib.rs:227`.

- [x] Definir um limite único e explícito para texto entregue/injetado.
- [x] Preservar o texto no histórico quando a injeção for recusada.
- [x] Se necessário, inserir por blocos com cancelamento e sem bloquear indefinidamente a aplicação ativa. N/A nesta implementação: o limite rígido de 10 000 caracteres recusa a entrega antes de gerar eventos, evitando um worker de longa duração.
- [x] Testar Unicode, emojis, multiline, texto muito longo e aplicação que rejeita eventos simulados.

### 2.4 Plataformas não Windows — decisão

- [x] Decidir se FamVoice é oficialmente Windows-only nesta fase.
- [x] Se for Windows-only, alinhar README, CI e release para o dizer explicitamente.
- [x] Se macOS/Linux forem suportados, substituir o histórico plaintext por armazenamento seguro e adicionar testes nesses sistemas. N/A: FamVoice fica oficialmente Windows-only e builds não Windows são recusados em compilação.

### Gate de saída

- Falhas de disco ou keyring nunca são reportadas como save bem-sucedido.
- Interromper uma escrita não destrói o último estado válido.
- A migração e a recuperação têm testes de falha, não apenas happy path.

---

## Fase 3 — tema, acessibilidade e UX do widget

### Objetivo

Corrigir os problemas P1 da interface sem alterar a identidade compacta da FamVoice.

### 3.1 Tema claro acidental — P1

Problema: `brand.css` ativa tokens claros quando o sistema está em light mode, mas a UI contém cores escuras hard-coded. O contraste pode aproximar-se de 1:1.

Pontos atuais: `src/assets/brand/brand.css:141`, `src/App.tsx:5` e `src/SettingsView.tsx:517`.

- [x] Correção curta recomendada: fixar explicitamente `data-theme="dark"` na app.
- [x] Adicionar teste que confirme o tema efetivo em sistema claro e escuro.
- [x] Não declarar suporte a light mode até todos os componentes usarem tokens semânticos.
- [x] Se light mode for posteriormente aprovado, tratá-lo como uma fase de produto separada. A política explícita em `src/theme.js` mantém essa mudança fora desta fase.

### 3.2 Modal de histórico — P1

- [x] Mover foco inicial para “Cancel” ou para a ação segura definida.
- [x] Conter Tab/Shift+Tab dentro do diálogo.
- [x] Fechar com Escape apenas quando não estiver a submeter.
- [x] Restaurar foco para “Clear history”.
- [x] Tornar o conteúdo atrás do modal indisponível para teclado/leitor de ecrã.

### 3.3 Captura de hotkeys — P1

- [x] Associar labels reais aos dois controlos de hotkey.
- [x] Anunciar o modo de captura e como cancelar/desativar.
- [x] Confirmar foco e nome acessível na árvore de acessibilidade automatizada; a prova com Narrator fica separada no smoke nativo.
- [x] Preservar captura de botões laterais do rato no hotkey principal.

### 3.4 Estados dinâmicos e erros — P1

- [x] Criar uma live region `polite` para recording/transcribing/success.
- [x] Usar `role="alert"` apenas para erros que exigem atenção imediata.
- [x] Evitar anúncios duplicados entre main view, toast e widget.
- [x] Tornar o resultado “Pasted to your app” / “Ready for paste-back” realmente visível; atualmente é calculado e depois escondido quando existe transcript.

### 3.5 Controles compactos e ações destrutivas — P2

- [x] Aumentar a área clicável de controlos de janela, histórico e glossário para pelo menos 24 px, mantendo ícones visualmente pequenos.
- [x] Adicionar undo para apagar uma entrada individual do histórico.
- [x] Corrigir a mensagem “Open Settings.” no widget: adicionar uma ação deliberada ou indicar “Tray menu → Settings”.

### 3.6 Performance e movimento — P2/P3

- [x] Impedir chamadas `cursorPosition()` sobrepostas no polling de 75 ms.
- [x] Avaliar eventos nativos enter/leave antes de manter polling contínuo. Não substituem o polling: uma WebView transparente com `setIgnoreCursorEvents(true)` não recebe enter/leave para voltar a ativar a interação; eventos de move/scale continuam a atualizar métricas e o polling single-flight trata a proximidade do cursor.
- [x] Substituir o kill switch global de movimento por alternativas direcionadas.
- [x] Trocar o easing overshoot de `src/components/VoiceWave.tsx:99` por um token sem bounce.

### Gate de saída

- Auditoria UI mínima: 17/20, sem P1 de acessibilidade ou tema.
- Navegação completa por teclado passa.
- Smoke com Windows em tema claro não altera o tema escuro efetivo da app.
- Widget mantém o tamanho e comportamento de recuperação atuais.

Ordem Impeccable sugerida: `$impeccable harden` → `$impeccable adapt` → `$impeccable optimize` → `$impeccable polish`; repetir `$impeccable audit` no final.

---

## Fase 4 — testes, dependências e automação

### Objetivo

Transformar os gates atuais em prova comportamental e desbloquear as atualizações de segurança.

### 4.1 Dependências vulneráveis

- [x] Atualizar `brace-expansion` para uma versão corrigida através do lockfile/override mínimo compatível.
- [x] Confirmar que continua apenas na cadeia dev e que `npm audit` passa.
- [x] Atualizar `serde_with` para `>=3.21.0` através da resolução Tauri compatível.
- [x] Reexecutar `cargo tree -i serde_with`, `cargo audit`, testes e Clippy.
- [x] Rever os 19 avisos `cargo audit` e documentar os que são apenas dependências Linux não atingíveis no alvo Windows.

### 4.2 Dependabot e workflows

- [x] Revalidar os PRs Dependabot abertos; os números observados na auditoria podem ter mudado.
- [x] Agrupar atualizações acopladas do CodeQL `init` e `analyze`.
- [x] Impedir configurações em que uma ação usa 4.37.0 e a outra executa 4.37.3.
- [x] Fazer os testes de workflow validar semântica e paridade, não apenas um SHA antigo literal.
- [x] Preservar pinning por SHA nas ações publicadas.
- [x] Confirmar que `cargo-audit` continua a usar instalação binária rápida/pinned em vez de compilação desnecessária.

### 4.3 Testes frontend comportamentais

Problema: a maioria dos 72 testes frontend lê source code e faz `assert.match`; isto não prova eventos, foco, rendering ou IPC.

- [x] Introduzir um runner de componentes compatível com React 19.
- [x] Criar mocks tipados para `invoke`, `listen`, updater e APIs de janela Tauri.
- [x] Cobrir modal/foco, hotkey capture, save errors, update states, live regions, história e widget.
- [x] Manter apenas os source checks que protegem invariantes realmente mecânicos; os fluxos principais passam agora pela suite de componentes e os checks estáticos restantes protegem wiring, assets, workflows e limites nativos.

### 4.4 Integração backend e smoke nativo

- [x] Adicionar servidor HTTP mock para sucesso, erro, timeout, retry, SSE fragmentado e payload vazio.
- [x] Adicionar fault injection para persistência e keyring.
- [x] Criar smoke Windows repetível para tray, hotkey, show/hide, monitor clamp, clipboard e Unicode multiline.
- [ ] Testar instalação/upgrade com um artefacto assinado anterior e o updater atual. Pendente de um updater publicado mais recente e de uma janela sem a instalação FamVoice ativa; o harness valida assinaturas/metadata e não instala nem termina processos automaticamente.

### Gate de saída

- `npm audit` e `cargo audit` não têm vulnerabilidades aplicáveis abertas.
- Dependabot consegue abrir PRs que passam CI quando a atualização é válida.
- Os principais fluxos UI são executados, não apenas encontrados no source.
- Existe um relatório separado do smoke nativo Windows.

---

## Fase 5 — modernizar a transcrição OpenAI

### Objetivo

Substituir o caminho OpenAI “legacy/fallback only” por uma integração atual, sem quebrar o Groq nem alterar silenciosamente preferências existentes.

Referência oficial: <https://developers.openai.com/api/docs/guides/speech-to-text>

### Tarefas

- [x] Confirmar novamente o catálogo oficial antes de editar; modelos e campos podem mudar.
- [x] Adicionar `gpt-transcribe` como opção recomendada para transcrição de ficheiro.
- [x] Manter `whisper-1` apenas para fallback/casos específicos suportados.
- [x] Tornar o multipart provider/model-aware:
  - `gpt-transcribe`: `languages`, `keywords`, `prompt` e streaming quando suportado;
  - modelos Whisper/Groq atuais: manter os campos compatíveis que já usam.
- [x] Mapear termos relevantes do glossário para `keywords`, sem enviar replacements nem dados irrelevantes.
- [x] Não enviar simultaneamente `language` e `languages`.
- [x] Validar parsing do evento final e deltas SSE fragmentados.
- [x] Definir timeouts com base no tamanho/duração do áudio, evitando um limite rígido demasiado curto para gravações longas.
- [x] Preservar utilizadores existentes através de migração explícita do model setting.
- [x] Atualizar Settings, defaults, validação Rust, testes, README e documentação de modelos em conjunto.
- [ ] Comparar precisão, latência e custo em amostras reais pt-PT antes de mudar o default.

### Gate de saída

- OpenAI e Groq passam a mesma matriz de testes de sucesso/erro/retry.
- O modelo recomendado não exige editar settings manualmente depois de uma atualização.
- Glossário e hints melhoram termos sem introduzir palavras não faladas.
- A escolha do novo default está apoiada por uma comparação registada.

---

## Fase 6 — novas funcionalidades

Estas funcionalidades só começam depois dos gates das Fases 1–5.

### 6.1 Retry last dictation — prioridade alta

- [x] Manter temporariamente apenas o último áudio falhado, com limite de tamanho e expiração curta.
- [x] Preferir RAM; se persistir em disco, cifrar e apagar de forma controlada.
- [x] Expor “Retry” sem obrigar o utilizador a repetir o ditado.
- [x] Invalidar o áudio quando o retry termina, expira ou o utilizador o elimina.
- [x] Não incluir áudio nos logs ou histórico textual.

### 6.2 Centro de diagnóstico — prioridade alta

- [x] Teste de microfone e nível de sinal.
- [x] Estado do dispositivo selecionado e deteção de desconexão.
- [x] Validação do hotkey e indicação de conflito/indisponibilidade.
- [x] Teste autenticado ao provider sem transcrever conteúdo pessoal.
- [x] Latência da última operação, versão e último erro sanitizado.
- [x] Exportação de diagnóstico sem transcrições nem secrets.

### 6.3 Histórico melhorado — prioridade média

- [x] Undo para delete individual.
- [x] Pesquisa local.
- [x] Favoritos/pins opcionais.
- [x] Exportação explícita para TXT/Markdown/JSON.
- [x] Controlos de retenção e “apagar tudo” coerentes com o modelo de privacidade.

### 6.4 Ditado longo/realtime — prioridade posterior

- [x] Tratar como modo opcional, não como substituição automática do push-to-talk atual.
- [x] Avaliar a API realtime recomendada no momento da implementação.
- [x] Definir partials, commit de turno, cancelamento, custos e recuperação de ligação.
- [ ] Provar primeiro que melhora significativamente a latência de ditados longos.

### Gate de saída

- Cada feature tem métrica de sucesso e teste nativo próprio.
- Não existe aumento não documentado de retenção de áudio ou texto.
- Funcionalidades opcionais não degradam o caminho simples de pressionar, falar e colar.

Estado em 2026-08-02: implementação e métricas/testes nativos definidos em [Phase 6 validation](phase-6-validation.md). O smoke interativo e o benchmark realtime continuam pendentes; por isso o gate de saída não é dado como concluído.

---

## Fase 7 — preparação e publicação da release

Executar apenas quando as fases incluídas na release estiverem concluídas e houver autorização explícita para publicar.

### Preparação

- [x] Garantir worktree limpo ou explicar cada ficheiro intencional. Worktree não está limpo por decisão do roadmap (nenhuma fase autorizou commit); os 70 caminhos estão classificados em “Classificação do worktree para release”.
- [x] Reexecutar todos os gates automatizados.
- [ ] Completar smoke nativo Windows, incluindo app instalada.
- [ ] Validar upgrade a partir de `0.3.29` ou da release estável imediatamente anterior.
- [x] Rever capabilities Tauri e schemas gerados se existirem novos comandos.
- [x] Confirmar que não há transcrições, keys ou artefactos de teste no pacote.

### Classificação do worktree para release

70 caminhos alterados sobre `4740bf8`; nenhum é acidental. Agrupamento por origem:

- Governance, segurança e workflows (6): `.github/SECURITY.md`, `.github/dependabot.yml`, `.github/workflows/ci.yml`, `.github/workflows/codeql.yml`, `.github/workflows/dependency-review.yml`, `.github/workflows/release.yml` — Fases 0 e 4.
- Dependências e tooling (7): `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts` — Fases 0, 4 e runner de componentes.
- Capabilities Tauri (3): `src-tauri/capabilities/default.json`, `settings.json` e `default.test.mjs` — Fase 0.
- Backend existente alterado (10): `audio.rs`, `clipboard.rs`, `glossary.rs`, `history.rs`, `injection.rs`, `input_hook.rs`, `lib.rs`, `prompt_optimizer/mod.rs`, `settings.rs`, `transcription.rs` — Fases 1, 2, 5 e 6.
- Backend novo (6): `delivery.rs`, `dictation.rs`, `persistence.rs` (Fases 1–2), `diagnostics.rs`, `retry_audio.rs`, `user_export.rs` (Fase 6).
- Frontend existente alterado (11): `App.css`, `MainView.tsx`, `SettingsView.tsx`, `WidgetView.tsx`, `appConstants.ts`, `appTypes.ts`, `main.tsx`, `components/Select.tsx`, `components/VoiceWave.tsx` e os testes de fonte `brandDesignAlignment`/`signalConsoleUi` — Fases 3, 5 e 6.
- Frontend novo (6): `theme.js`, `theme.d.ts`, `diagnostics/DiagnosticsPanel.tsx`, `diagnostics/types.ts`, `history/HistoryPrivacyPanel.tsx` e `test/tauriMocks.ts` — Fases 3, 4 e 6.
- Testes novos e atualizados (8): `MainView.component.test.tsx`, `SettingsView.component.test.tsx`, `phase3Ui.test.mjs`, `theme.test.mjs`, `ciWorkflow.test.mjs`, `releaseWorkflow.test.mjs`, `widgetBehavior.test.mjs`, `windowVisibility.test.mjs` — Fases 3 e 4.
- Documentação e utilitários de prova (13): `README.md`, `ARCHITECTURE.md`, `docs/famvoice-remediation-roadmap.md`, `docs/phase-6-long-dictation-spike.md`, `docs/phase-6-validation.md`, `docs/security/rust-audit-warnings.md`, `docs/transcription-models.md`, `docs/transcription/phase5-pt-pt-evaluation.md`, `docs/windows-native-smoke.md`, `docs/windows-native-smoke-latest.md`, `scripts/transcription-evaluation.ps1`, `scripts/windows-native-smoke.ps1`, `src-tauri/examples/verify_updater_signature.rs`.

Nenhum ficheiro gerado entra no worktree: `dist/`, `node_modules/`, `src-tauri/target/` e `src-tauri/gen/` continuam ignorados, e `src-tauri/gen/schemas` não é versionado.

### Versionamento

- [ ] Escolher a nova versão sem reutilizar `0.3.29`.
- [ ] Sincronizar `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` e `src-tauri/tauri.conf.json`.
- [ ] Criar `docs/releases/v<versão>.md` com notas obrigatórias orientadas ao utilizador.
- [ ] Organizar notas em `### Fixed`, `### Added` e `### Changed` conforme aplicável.

### Publicação

- [ ] Commit final revisto e autorizado.
- [ ] Push e confirmação do SHA remoto.
- [ ] Tag `v<versão>` criada a partir do commit correto.
- [ ] Workflow de release concluído com sucesso.
- [ ] EXE, MSI, assinaturas e `latest.json` presentes.
- [ ] Versão e hashes do instalador confirmados.
- [ ] Updater testado a partir da versão anterior.

### Gate final

- Validação automatizada: concluída.
- Smoke nativo: concluído e documentado.
- Estado remoto/release: concluído e verificado.
- Migração de dados: concluída ou marcada como não aplicável.

---

## Validação automatizada comum

Executar na raiz do repositório, salvo indicação em contrário:

```powershell
npm test
npm run build
npm run lint
npm audit --audit-level=moderate
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo audit --file src-tauri/Cargo.lock
git diff --check
```

Quando existirem alterações a workflows, validar também os testes de CI/release e o resultado efetivo no GitHub.

## Matriz mínima de smoke nativo Windows

- [ ] Primeiro arranque e abertura de Settings.
- [ ] Configurar/alterar API key sem a expor.
- [ ] Selecionar microfone e gravar.
- [ ] Dois ditados rápidos com respostas fora de ordem simuladas.
- [ ] Ditado sem voz e resposta vazia simulada.
- [ ] Falha de rede, timeout e retry.
- [ ] Auto-paste com clipboard copy ligado e desligado.
- [ ] Unicode, emojis, multiline e texto longo.
- [ ] Re-paste rápido duas vezes, preservando o clipboard original.
- [ ] Widget hide/restore por botão, tray e hotkey.
- [ ] Recuperação de widget fora do ecrã/monitor removido.
- [ ] Windows em tema claro e escuro.
- [ ] Navegação por teclado, foco modal e leitor de ecrã.
- [ ] Update disponível, instalação, restart e versão final.

## Registo de execução

Preencher uma entrada por fase:

```markdown
### Fase N — <nome>

- Data:
- Commit base:
- Branch:
- Alterações preexistentes preservadas:
- Tarefas concluídas:
- Validação automatizada:
- Smoke nativo:
- Estado remoto/release:
- Migração de dados:
- Riscos ou trabalho restante:
- Commit final da fase:
```

### Fase 0 — consolidar a base de trabalho

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; 18 caminhos classificados acima. Foi mantida a salvaguarda reversível `phase-0-preserve-before-4740bf8-2026-08-02` no stash.
- Tarefas concluídas: auditoria dos diffs; integração por fast-forward de `4740bf8`; reaplicação sem conflitos; preservação da instalação binária/pinned de `cargo-audit`; paridade `0.3.29` nos cinco manifestos; teste de workflow tornado compatível com LF e CRLF.
- Validação automatizada: `npm test` 73/73; build e lint passam; `cargo fmt`, `cargo check`, 111 testes Rust e Clippy passam; `cargo audit` passa com 19 warnings permitidos; `git diff --check` passa. `npm audit --audit-level=moderate` mantém apenas `brace-expansion@5.0.7` high, pela cadeia dev ESLint → minimatch, catalogado para a Fase 4.
- Smoke nativo: N/A; a Fase 0 consolida Git e não altera comportamento nativo. Não foi terminada nem aberta nenhuma instância FamVoice.
- Estado remoto/release: PR #53 e commit `4740bf8` integrados; checks remotos CI/CodeQL do PR passaram; zero alertas CodeQL abertos; release estável `v0.3.29` continua publicada com EXE, MSI, assinaturas e `latest.json`. Alterações locais não publicadas; nenhuma tag ou release criada nesta fase.
- Migração de dados: N/A.
- Riscos ou trabalho restante: corrigir `brace-expansion` e o alerta Dependabot medium #29 de `serde_with` na Fase 4; os 19 warnings transitivos de `cargo audit` continuam reservados à revisão de aplicabilidade dessa fase; validar no GitHub as alterações locais de workflow apenas quando houver autorização para commit/push.
- Commit final da fase: não criado; o roadmap proíbe commit sem autorização explícita.

### Fase 2 — persistência, segredos e recuperação

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; o hardening da Fase 0 e o trabalho concorrente da Fase 1 foram mantidos. As integrações partilhadas em `lib.rs`, `SettingsView.tsx` e `appTypes.ts` foram feitas sobre o estado corrente, sem reset nem formatação global.
- Tarefas concluídas: helper `AtomicFile` partilhado com temporário no mesmo diretório, sync e replace Windows atómico; backup `.bak` substituído atomicamente; revisões que rejeitam snapshots atrasados; recuperação de último estado válido; migrações de settings/histórico sem conservar plaintext; backend de credenciais injetável, rollback de escrita parcial e autoridade keyring → fallback DPAPI; estado de recuperação sanitizado na Settings; limite único de 10 000 caracteres para entrega/re-paste/injeção direta com preservação no histórico; decisão e enforcement Windows-only em código, README, CI e release.
- Validação automatizada: `npm test` 73/73; build e lint passam; `cargo fmt --check`, `cargo check`, 140 testes Rust e Clippy com `-D warnings` passam; `cargo audit` passa com os 19 warnings transitivos já catalogados; `git diff --check` passa. `npm audit --audit-level=moderate` continua vermelho apenas para `brace-expansion@5.0.7` high na cadeia dev ESLint → minimatch, reservado à Fase 4.
- Smoke nativo: não executado. Foi detetada uma instância instalada ativa em `C:\Program Files\FamVoice\famvoice.exe`; não foi terminada nem sobreposta por um dev build, para não interferir com hotkeys ou uma sessão em uso. A Settings nativa, a alteração real de API key e a injeção num processo externo continuam por provar manualmente.
- Estado remoto/release: apenas local; nenhum commit, push, tag ou release criado.
- Migração de dados: caminhos automáticos de settings plaintext e histórico plaintext → DPAPI cobertos por testes isolados; não executados contra os dados da instalação ativa.
- Riscos ou trabalho restante: smoke nativo Windows isolado quando não houver uma instância em uso; o gate conhecido de `brace-expansion` e a revisão dos warnings Rust continuam na Fase 4.
- Commit final da fase: não criado; sem autorização explícita para commit.

### Fase 1 — integridade do ciclo de ditado

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; o conjunto da Fase 0 foi mantido. Foram também preservadas alterações concorrentes de fases posteriores observadas durante a execução; só foram feitos ajustes mínimos de integração/Clippy nos pontos sobrepostos.
- Tarefas concluídas: coordenador monotónico de sessões com política `latest session wins`; IDs ligados a Start/Stop, transcrição, eventos e entrega; resultados ultrapassados guardados apenas no histórico; entrega final serializada; respostas vazias rejeitadas antes de efeitos laterais nos caminhos normal e streaming; re-paste transformado numa transação serializada com snapshot por operação; falha CPAL encaminhada uma vez para a orquestração com reconstrução no Start seguinte; troca de microfone rejeitada no backend e desativada/explicada em Settings durante gravação ou transcrição.
- Validação automatizada: `npm test` 73/73, build e lint passam; `cargo fmt`, `cargo check`, 140 testes Rust e Clippy com `-D warnings` passam; `cargo audit` passa com os 19 warnings permitidos já catalogados; `git diff --check` passa; detector Impeccable dos dois alvos UI sem findings. `npm audit --audit-level=moderate` mantém apenas `brace-expansion@5.0.7` high, reservado à Fase 4.
- Smoke nativo: pendente. Existe uma instância instalada `0.3.29` ativa em `C:\Program Files\FamVoice\famvoice.exe --minimized`; não foi terminada nem foi aberta uma segunda instância que pudesse disputar hotkeys, microfone, clipboard ou dados. Continuam por provar nativamente dois ditados rápidos fora de ordem, falha real do dispositivo e re-pastes rápidos.
- Estado remoto/release: não alterado; nenhum commit, push, tag ou release criado.
- Migração de dados: N/A; a Fase 1 não altera formatos persistidos.
- Riscos ou trabalho restante: completar o smoke nativo da Fase 1 numa janela sem sessão FamVoice ativa. Os cenários concorrentes estão cobertos por testes simulados, mas não contam como prova nativa.
- Commit final da fase: não criado; sem autorização explícita para commit.

### Fase 3 — tema, acessibilidade e UX do widget

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; o trabalho local das Fases 0–2 foi mantido. O runner comportamental e as atualizações de dependências/workflows que surgiram concorrentemente foram preservados e integrados apenas nos pontos necessários para validar a Fase 3, sem reset nem formatação global.
- Tarefas concluídas: política explícita de tema dark aplicada antes do primeiro render e testada contra preferências light/dark; modal de clear-history com foco seguro, trap, Escape condicionado, restituição de foco e fundo `inert`; hotkeys com labels, instruções, captura cancelável e botões laterais preservados; live regions e alerts sem duplicação; resultado de entrega visível; alvos compactos mínimos de 24 px; undo persistido de delete individual; copy do widget corrigido para `Tray menu → Settings`; polling do cursor single-flight com justificação para manter polling; reduced motion direcionado e easing sem overshoot. O audit final Impeccable ficou em 18/20 — acessibilidade 4, performance 4, theming 3, responsive/contexto desktop 3 e integridade 4 — sem findings P1; o detector automático Impeccable não reportou findings determinísticos nos alvos alterados.
- Validação automatizada: `npm test` passa com 84 source checks e 8 testes comportamentais React; build e lint passam; `npm audit --audit-level=moderate` passa com zero vulnerabilidades; `cargo fmt --check`, `cargo check`, 141 testes Rust e Clippy com `-D warnings` passam; `cargo audit` passa com os 19 warnings transitivos já catalogados; `git diff --check` passa. O contraste de copy secundária pequena foi medido em 7.48:1 (`slate-400` sobre `#0f0f0f`) e protegido por teste.
- Smoke nativo: pendente. A instalação `0.3.29` continua ativa em `C:\Program Files\FamVoice\famvoice.exe --minimized` (PID 755124); não foi terminada nem foi aberta uma segunda instância que pudesse disputar hotkeys, widget, microfone, clipboard ou dados. Permanecem por provar no build desta fase Windows light/dark, percurso completo por teclado, Narrator, undo real e preservação do tamanho/recuperação do widget.
- Estado remoto/release: apenas local; nenhum commit, push, tag ou release criado.
- Migração de dados: N/A; o undo reutiliza o envelope de histórico atual sem alterar o formato persistido.
- Riscos ou trabalho restante: o gate formal da fase continua dependente do smoke nativo numa janela sem a instalação FamVoice em uso. Light mode continua deliberadamente não suportado e deverá ser uma fase de produto separada se vier a ser aprovado.
- Commit final da fase: não criado; sem autorização explícita para commit.

### Fase 4 — testes, dependências e automação

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; foram preservadas as Fases 0–2 e a Fase 3 executada concorrentemente. A Fase 4 limitou-se a dependências/lockfiles, workflows, infraestrutura de testes, integração HTTP e documentação/scripts novos; não reverteu nem formatou globalmente a UI da Fase 3.
- Tarefas concluídas: override mínimo `brace-expansion@5.0.8`, ainda apenas em ESLint → minimatch; resoluções `serde_with@3.21.0` e `memmap2@0.9.11` (correção de `RUSTSEC-2026-0186` detetada ao refrescar a base); revisão dos 19 warnings Rust restantes em `docs/security/rust-audit-warnings.md`; revalidação ao vivo de 14 PRs Dependabot; agrupamento CodeQL e paridade `init`/`analyze` em `v4.37.3` com SHA imutável; testes de workflow sem SHA antigo literal; Vitest/happy-dom compatível com React 19, mocks tipados Tauri e oito testes comportamentais; servidor HTTP local com sucesso, erro, timeout/retry, SSE fragmentado e vazio; reaproveitamento do fault injection de persistência/keyring já integrado; harness e relatório separado de smoke Windows.
- Validação automatizada: `npm test` passa com 84 source checks mecânicos e 8 testes de componentes; build e lint passam; `npm audit --audit-level=moderate` passa com zero vulnerabilidades e confirma `brace-expansion@5.0.8` apenas na cadeia dev; `cargo fmt --check`, `cargo check`, 147 testes Rust e Clippy com `-D warnings` passam; `cargo audit` passa com zero vulnerabilidades e 19 warnings revistos; `cargo tree -i serde_with` confirma `3.21.0`; `git diff --check` passa.
- Smoke nativo: apenas preflight, registado em `docs/windows-native-smoke-latest.md`. Encontrou dois monitores e um binário release local antigo `0.3.28`; confirmou a metadata publicada `0.3.29` e verificou criptograficamente o instalador `0.3.29` com a assinatura destacada Tauri e a chave do `tauri.conf.json`. Recusou o smoke interativo porque a instalação `0.3.29` está ativa em `C:\Program Files\FamVoice\famvoice.exe --minimized` (PID 755124). Nenhum processo foi terminado, substituído ou instalado.
- Estado remoto/release: 14 PRs Dependabot abertos revalidados; 10 tinham checks sem falhas e quatro tinham falhas (#68/#70 por CodeQL desacoplado, #67 pelo antigo literal do install-action e #66 pela atualização `cpal`). As correções são apenas locais; o alerta remoto #29 de `serde_with` só fechará depois de publicação/merge. Nenhum commit, push, tag ou release criado.
- Migração de dados: N/A; a Fase 4 não altera formatos persistidos.
- Riscos ou trabalho restante: executar o smoke interativo quando não existir uma sessão FamVoice ativa; testar instalação/upgrade apenas quando houver um artefacto anterior assinado e um updater publicado para uma versão mais recente; validar os workflows no GitHub depois de autorização para commit/push. Estes limites mantêm o gate nativo/remoto formal ainda pendente.
- Commit final da fase: não criado; sem autorização explícita para commit.

### Fase 5 — modernizar a transcrição OpenAI

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; todo o trabalho local das Fases 0–4 foi mantido. Surgiram também, durante esta execução, ficheiros concorrentes da Fase 6 (`diagnostics.rs`, `retry_audio.rs`, `user_export.rs` e documentação associada); não foram apagados, reformatados nem integrados artificialmente na Fase 5.
- Tarefas concluídas: catálogo e contrato oficial OpenAI revalidados; `gpt-transcribe` adicionado como default recomendado e `whisper-1` mantido como fallback especializado; capabilities multipart explícitas por provider/model; `languages[]`, `keywords[]`, contexto e streaming apenas no caminho suportado; `language` singular e contrato existente preservados em Whisper/Groq; parser SSE byte-safe com deltas, evento final e frame EOF; timeout escalável por duração/tamanho; keywords limitadas aos targets literais e filtrados do glossário, sem replacements; migração versionada v1 com aviso sanitizado e preservação de escolhas explícitas pós-migração/Groq; Settings, testes, README, arquitetura e documentação de modelos sincronizados; harness pt-PT opt-in criado com preflight sem rede, métricas agregadas, custo estimado e deteção de ocorrência de keywords não faladas.
- Validação automatizada: `npm test` passa com 84 source checks e 12 testes React; build e lint passam; `npm audit --audit-level=moderate` passa com zero vulnerabilidades; `cargo check` e 190 testes Rust passam; `cargo audit` passa com zero vulnerabilidades e os 19 warnings transitivos já revistos; `git diff --check` passa. `rustfmt --check` passa nos três módulos próprios da Fase 5 (`transcription.rs`, `glossary.rs`, `settings.rs`); o `cargo fmt --check` global encontra apenas formatação pendente nos hunks concorrentes de `lib.rs`/`user_export.rs`. O Clippy estrito comum fica vermelho por três findings desse mesmo trabalho da Fase 6 (dois `dead_code` e um `clippy::nonminimal_bool` em `retry_audio.rs`); com apenas esses lints permitidos, todo o target passa sem finding adicional na Fase 5.
- Smoke nativo: não executado. A instalação `0.3.29` permanece ativa em `C:\Program Files\FamVoice\famvoice.exe --minimized` (PID 755124); não foi terminada nem foi aberta uma segunda instância que pudesse disputar hotkeys, microfone ou settings. A seleção/migração visível e um ditado real por provider continuam por provar no build desta fase.
- Estado remoto/release: apenas local; nenhum commit, push, tag ou release criado.
- Migração de dados: `transcription_model_settings_version=1` coberto por testes: OpenAI legado migra uma vez para `gpt-transcribe` com aviso; `whisper-1` escolhido depois da migração e ambos os modelos Groq permanecem estáveis. A migração não foi executada contra os dados da instalação ativa.
- Riscos ou trabalho restante: a comparação paga em amostras reais pt-PT não foi executada por falta de corpus consentido e credenciais no ambiente; o preflight marca corretamente as quatro variantes como `not measured`. Por isso o item de comparação e o gate final de escolha do default continuam abertos. É também necessário repetir o Clippy estrito depois de a Fase 6 concorrente ficar completamente ligada e executar smoke nativo numa janela sem FamVoice ativa.
- Commit final da fase: não criado; sem autorização explícita para commit.

### Fase 6 — novas funcionalidades

- Data: 2026-08-02
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; as Fases 0–4 e a Fase 5 executada concorrentemente foram mantidas. A integração reutilizou o contrato atualizado `TranscriptionRequest` e fez alterações estreitas nos ficheiros partilhados, sem reset, staging ou formatação global do restante worktree.
- Tarefas concluídas: cache RAM-only, single-use, de um áudio falhado com 10 MiB/120 s e scrub no drop; retry integrado no painel e widget com settings atuais e sessão monotónica; centro de diagnóstico com teste agregado de microfone, dispositivo/stream, registo/conflito de hotkeys, `GET /models` autenticado sem conteúdo e export allowlisted; histórico cifrado v2 com migração, pesquisa local, pins persistidos, exports TXT/Markdown/JSON sem overwrite, retenção 0/25/50/100 e purge crash-safe; métricas e smoke por feature documentados; spike realtime atual documentado como opt-in sem ativar qualquer caminho de produto.
- Validação automatizada: `npm test` passa com 84 source checks e 16 testes React; build e lint passam; `npm audit --audit-level=moderate` passa com zero vulnerabilidades; `cargo fmt --check`, `cargo check`, 191 testes Rust e Clippy estrito passam; `cargo audit` passa com zero vulnerabilidades e os 19 warnings transitivos já revistos; `git diff --check` passa; detector Impeccable nos cinco alvos UI não reporta findings.
- Smoke nativo: apenas preflight em `docs/windows-native-smoke-latest.md`. Confirmou dois monitores, mas bloqueou a sessão exclusiva porque a instalação `0.3.29` continua ativa (PID 755124); nenhum processo foi terminado. O EXE release local encontrado é antigo (`0.3.28`) e não serve como prova da Fase 6. Todos os checks interativos específicos continuam pendentes.
- Estado remoto/release: apenas local; nenhum commit, push, tag, PR ou release criado.
- Migração de dados: histórico plaintext/v1 → envelope cifrado v2 adiciona `pinned=false` e retenção default 100; round-trip, backup e purge interrompido cobertos por testes. Não foi executada contra os dados da instalação ativa. O áudio de retry não tem migração nem persistência.
- Riscos ou trabalho restante: o gate formal continua aberto até existir smoke nativo do build atual numa janela sem FamVoice ativa. O benchmark pt-PT realtime não foi executado; não existe implementação realtime nem decisão go. A Fase 6 foi implementada a pedido explícito enquanto os gates nativos anteriores permanecem pendentes, sem os declarar concluídos.
- Commit final da fase: não criado; sem autorização explícita para commit.

### Fase 7 — preparação da release (parcial, offline)

- Data: 2026-08-03
- Commit base: `4740bf8e3f516f1296343df862e60a669c933e53`
- Branch: `master`, alinhado com `origin/master`
- Alterações preexistentes preservadas: sim; nenhuma alteração de comportamento da app foi feita nesta execução. As edições limitam-se a esta secção, às caixas/classificação da Fase 7 e ao alinhamento do gate Clippy em CI/release com o respetivo teste.
- Tarefas concluídas: reexecução completa dos gates automatizados; revisão das capabilities Tauri contra os comandos novos; verificação de higiene do pacote; classificação dos 70 caminhos do worktree; Clippy do CI e da release alinhado com o gate local `--all-targets --all-features`, protegido por um teste de paridade em `src/ciWorkflow.test.mjs`.
- Validação automatizada: `npm test` passa com 85 source checks e 16 testes de componentes; `npm run build` e `npm run lint` passam; `npm audit --audit-level=moderate` reporta zero vulnerabilidades; `cargo fmt --check`, 191 testes Rust e Clippy `--all-targets --all-features -- -D warnings` passam; `cargo audit` passa com zero vulnerabilidades e os 19 warnings transitivos já revistos; `git diff --check` passa. Os avisos LF→CRLF do `git diff --check` são de normalização de finais de linha, não erros de whitespace.
- Capabilities e schemas: os 26 comandos de `generate_handler`, incluindo os sete novos das Fases 5–6 (`get_retry_audio_state`, `retry_last_dictation`, `discard_last_failed_dictation`, `get_diagnostics_snapshot`, `run_microphone_test`, `test_provider_auth`, `export_diagnostics`), são comandos da aplicação e não exigem entradas de ACL; só comandos `core:` e de plugin são governados por capabilities. Não foram adicionados plugins nem janelas: o frontend continua a usar apenas `plugin-updater`, `plugin-autostart` e `plugin-process`, e as janelas continuam a ser `main` e `settings`, cada uma com a sua capability restrita. Os exports das Fases 5–6 são escritos em Rust, sem plugin `fs`/`dialog`. `src-tauri/gen/schemas` é regenerado no build e não é versionado.
- Higiene do pacote: `bundle` não declara `resources`, pelo que só o binário e os ícones são empacotados; `scripts/`, `src-tauri/examples/` e `docs/` ficam fora do instalador. Não existem ficheiros de áudio nem transcrições no repositório. A varredura de segredos só encontra sentinelas dentro de `#[cfg(test)]` (`diagnostics.rs`, `settings.rs`, `lib.rs`) e placeholders explícitos na documentação de avaliação pt-PT; `.gitignore` mantém `.env*`, `*.key`, `*.pem`, `*.p12`, `*.sigkey`, `history.json` e `settings.json` fora do controlo de versões.
- Versionamento: não iniciado. Os cinco manifestos continuam coerentes em `0.3.29`; a nova versão exige decisão explícita.
- Smoke nativo: não executado. A instalação `0.3.29` continua ativa em `C:\Program Files\FamVoice\famvoice.exe` (PID 755124); nenhum processo foi terminado. O smoke interativo e a validação de upgrade continuam bloqueados.
- Estado remoto/release: inalterado; nenhum commit, push, tag ou release criado.
- Migração de dados: N/A nesta execução.
- Riscos ou trabalho restante: as duas caixas de preparação bloqueadas dependem de uma janela sem FamVoice em uso; versionamento e publicação continuam por autorizar. O Clippy estrito compila também os targets de teste, o que acrescenta algum tempo ao pipeline reduzido em `4740bf8`; o cache Rust já partilhado entre passos limita o custo e o efeito real só é observável depois de autorização para push.
- Commit final da fase: não criado; sem autorização explícita para commit.

## Ordem recomendada

1. Fase 0 — consolidar a base.
2. Fase 1 — corrigir integridade do ditado.
3. Fase 2 — tornar persistência e secrets resistentes a falhas.
4. Fase 3 — corrigir tema e acessibilidade.
5. Fase 4 — reforçar testes, dependências e automação.
6. Fase 5 — modernizar OpenAI transcription.
7. Fase 6 — adicionar features por valor.
8. Fase 7 — publicar apenas após autorização e prova nativa.
