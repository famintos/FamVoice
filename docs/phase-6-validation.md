# Fase 6 — métricas e validação

Data: 2026-08-02. Este documento separa implementação, prova automatizada e prova nativa. Não atribui resultados a testes que ainda não foram executados num build Windows desta fase.

## Retry do último ditado falhado

| Critério | Métrica de sucesso | Prova automatizada | Prova nativa |
|---|---|---|---|
| Retenção limitada | no máximo um áudio, até 10 MiB e 120 s | testes de limite, substituição e expiração em `retry_audio.rs` | `retry-privacy-lifecycle` |
| Recuperação | retry reutiliza o áudio sem nova captura e entrega exatamente uma vez | sessão monotónica e `take()` single-use | `retry-last-dictation` |
| Invalidação | retry, discard, nova gravação, expiração e encerramento deixam o estado indisponível | testes de consumo, geração e `Drop` | `retry-privacy-lifecycle` |
| Privacidade | zero ficheiros, logs, histórico ou DTO com áudio/metadados sensíveis | cache sem path/serde de bytes; estado público expõe apenas `available` | inspeção de app-data e restart em `retry-privacy-lifecycle` |

O retry usa sempre as settings atuais. Uma falha no próprio retry não volta a reter o áudio e não pode produzir uma segunda entrega.

## Centro de diagnóstico

| Critério | Métrica de sucesso | Prova automatizada | Prova nativa |
|---|---|---|---|
| Microfone | teste exclusivo devolve RMS/peak agregados e deteta ausência de sinal | testes do actor de áudio e redução sem PCM | `diagnostics-microphone-device` |
| Dispositivo | seleção ausente aparece como desconectada; saúde do stream é visível | testes de snapshot sem ID | disconnect/reconnect em `diagnostics-microphone-device` |
| Hotkeys | sintaxe, registo real e conflito são distinguidos | testes de conflito e helpers de registo | `diagnostics-hotkey-provider` |
| Provider | autenticação usa `GET /models`, sem body, áudio ou transcript | servidor HTTP local verifica método, auth e body vazio | `diagnostics-hotkey-provider` |
| Operações | versão, latência e erro allowlisted refletem a operação monotónica mais recente | teste de respostas fora de ordem e sanitização | ditado + testes no painel |
| Export | zero keys, transcripts, áudio, IDs/nomes de dispositivo ou atalhos literais | teste de allowlist com sentinelas | `diagnostics-export-privacy` |

## Histórico e privacidade

| Critério | Métrica de sucesso | Prova automatizada | Prova nativa |
|---|---|---|---|
| Pesquisa | filtro local Unicode/case-insensitive sem chamada de rede | teste de componente | `history-search-pin-export` |
| Pins | pin persiste sem alterar a ordem canónica guardada | round-trip cifrado e teste de componente | restart em `history-search-pin-export` |
| Export | TXT, Markdown e JSON são explícitos, determinísticos e nunca sobrescrevem | testes de formato e `create_new` | inspeção dos três ficheiros em `history-search-pin-export` |
| Retenção | opções 0/25/50/100; backend rejeita qualquer valor acima de 100 | testes de default, zero, redução e limite máximo | criação de entradas em `history-retention-purge` |
| Apagar tudo | main, `.bak`, `.corrupt*`, temporários e purge interrompido não recuperam texto apagado | testes de purge crash-safe e restart | `history-retention-purge` |

Reduzir a retenção não elimina entradas existentes silenciosamente. O valor limita novas adições; apagar conteúdo existente continua a ser uma ação explícita. Exports são plaintext fora do armazenamento gerido por FamVoice e não são apagados pelo purge.

## Ditado longo/realtime

O spike [Fase 6.4 — ditado longo/realtime](phase-6-long-dictation-spike.md) define opt-in, máquina de estados, partials transitórios, commit/cancel, reconexão, backpressure, custo e o corpus pt-PT. O caminho realtime não foi implementado nem ativado.

O resultado permanece **no decision** até o benchmark real demonstrar todos os thresholds do spike, incluindo pelo menos 30% de melhoria no p95 `commit → paste` para ditados de 2–10 minutos, sem regressão material de qualidade, integridade ou privacidade.

## Gate e comandos

Validação comum:

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

O smoke Windows é executado por `scripts/windows-native-smoke.ps1` e segue [as instruções nativas](windows-native-smoke.md). Um preflight não substitui os checks interativos. A instalação ativa nunca deve ser terminada para abrir uma segunda instância concorrente.
