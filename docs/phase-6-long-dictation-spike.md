# Fase 6.4 — spike de ditado longo/realtime

> Estado: proposta para benchmark; não implementada nem validada nativamente.  
> Data da revisão documental: 2026-08-02.  
> Resultado go/no-go: **por determinar**. Este documento não contém resultados medidos.

## Decisão a testar

O modo realtime deve ser uma opção explícita para ditados longos. Não substitui o push-to-talk atual nem altera a preferência/provider de um utilizador existente. O caminho atual de gravar, transcrever o ficheiro e colar continua a ser o fallback e a referência do benchmark.

A hipótese principal é uma sessão de transcrição Realtime por WebSocket, mantida no backend Rust. A documentação oficial recomenda `gpt-live-transcribe` quando são necessários deltas à medida que a fala chega e um resultado final no commit do turno. `gpt-transcribe` em Realtime fica apenas como variante de benchmark para transcrição que começa depois de um commit manual; não é live captioning.

Referências oficiais a revalidar antes de qualquer implementação:

- [Speech-to-text](https://developers.openai.com/api/docs/guides/speech-to-text), a referência já usada no roadmap para o caminho de ficheiro;
- [Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription), incluindo modelos, deltas, commit e contexto;
- [Realtime via WebSocket](https://developers.openai.com/api/docs/guides/realtime-websocket), adequado ao pipeline de áudio no backend Rust;
- [Voice activity detection](https://developers.openai.com/api/docs/guides/realtime-vad), apenas para a variante futura com turnos automáticos.

Modelos, eventos, preços e campos de configuração são superfície dinâmica. Devem ser novamente confirmados nestas páginas no início da implementação; não ficam congelados por este spike.

## Âmbito do primeiro protótipo

- Opt-in denominado “Ditado longo (experimental)”, desligado por defeito.
- Uma sessão de transcrição, sem resposta de voz/assistente e sem ferramentas.
- WebSocket iniciado e terminado pelo backend Rust; a WebView recebe apenas estado e texto.
- Áudio PCM no formato exigido pela sessão, produzido a partir do pipeline de captura existente.
- `turn_detection: null` no primeiro protótipo: o utilizador faz commit de forma deliberada.
- `gpt-live-transcribe` como primeira variante; `gpt-transcribe` committed-turn e o upload atual são comparadores.
- `languages: ["pt"]` e termos literais do glossário em `keywords`, depois da mesma sanitização prevista para a Fase 5. Nunca enviar `language` e `languages` em simultâneo.

Ficam fora do primeiro protótipo VAD automático, diarização, timestamps por palavra, resposta áudio, conversação, persistência de áudio e substituição do provider atual.

## Identidade e máquina de estados

Cada execução recebe um `dictation_session_id` monotónico local. Cada commit recebe um `turn_id` local e fica associado ao `item_id` devolvido pela API. Eventos sem a identidade ativa ou pertencentes a uma geração de ligação anterior são ignorados para UI, histórico e paste.

```text
Disabled
   | opt-in
   v
Idle -> Connecting -> Streaming -> Committing -> Finalizing -> Completed -> Idle
          |              |             |             |
          +--------------+-------------+-------------+--> Cancelling -> Idle
                         |
                         +--> Reconnecting -> Streaming
                                   |
                                   +--> FallbackPending -> Finalizing/Failed

Qualquer estado ativo -> Failed -> Idle ou Retry explícito
```

Regras de transição:

- `Connecting` só passa a `Streaming` depois da configuração efetiva da sessão ser confirmada.
- `Streaming` aceita áudio e partials da geração ativa; `Committing` fecha a entrada desse turno.
- `Finalizing` termina apenas com o evento final correspondente ao `item_id`. A documentação avisa que eventos finais de turnos diferentes podem chegar fora de ordem, pelo que a ordem de chegada nunca é autoridade.
- `Completed` grava uma única entrada de histórico e entrega uma única vez. A entrega continua serializada pelo coordenador de ditado existente.
- `Cancel`, falha terminal ou nova sessão invalidam imediatamente geração, partials e callbacks anteriores.

## Partials, commit e resultado final

Partials são conteúdo provisório e substituível:

- existem apenas em memória e numa região visual marcada como provisória;
- são reconciliados por `item_id`/`content_index`, nunca concatenados cegamente entre turnos;
- podem ser revistos por deltas posteriores;
- nunca são copiados, colados, guardados no histórico, exportados ou usados pelo prompt optimizer;
- desaparecem em cancelamento, erro terminal ou conclusão do turno.

O commit inicial é manual: libertar/confirmar o controlo de ditado longo deixa de aceitar novas amostras, envia o commit uma única vez e aguarda o final. Cliques/hotkeys repetidos são idempotentes. Só o transcript final, validado como não vazio, entra no fluxo normal de glossary/finalização, histórico e entrega.

O botão Cancel fica disponível em `Connecting`, `Streaming`, `Reconnecting`, `Committing` e `Finalizing`. Cancelar invalida a sessão local antes de fechar/limpar o buffer remoto. Um final tardio nunca reativa a sessão nem produz paste. Se o cancelamento remoto não puder ser confirmado, a UI continua cancelada localmente e a ligação é encerrada.

## Reconexão e fallback

A reconexão usa backoff com jitter, número de tentativas e duração total limitados. Não existe retry infinito escondido.

- Antes do commit: pode abrir-se uma sessão nova e reenviar apenas áudio ainda presente no buffer local da mesma sessão, desde o início do turno. A UI mantém o estado `Reconnecting`; partials da ligação anterior são descartados.
- Depois de o commit ser enviado: o estado é ambíguo. Não se repete automaticamente o commit nem se juntam resultados de duas ligações. Se o áudio completo desse turno ainda estiver na RAM, oferece-se fallback explícito para transcrição de ficheiro; caso contrário apresenta-se erro recuperável e preserva-se qualquer final já confirmado.
- Depois de um final confirmado: a ligação pode fechar sem afetar o resultado. O identificador do turno impede duplicação.
- Troca de rede, sleep/resume e mudança de microfone entram na matriz de falhas do benchmark.

## Backpressure e memória

O callback de captura nunca espera pela rede. Áudio passa por uma fila limitada, medida em milissegundos de PCM e com contadores de frames/bytes. O limite exato é escolhido pelo benchmark, não por suposição neste documento.

1. O produtor coloca blocos numerados numa fila bounded sem bloquear.
2. Um único consumidor agrega blocos até ao tamanho de envio medido como eficiente e envia-os por ordem.
3. Ao atingir o aviso de high-water, a UI indica degradação de rede e deixa de aceitar crescimento ilimitado.
4. Ao atingir o limite duro, a sessão deixa de enviar áudio novo, invalida partials e transita para fallback/erro. Nunca descarta silenciosamente o bloco mais antigo ou intermédio.
5. Métricas registam apenas duração, contagens, tamanhos e latência; não áudio nem texto.

O protótipo deve demonstrar memória limitada durante pelo menos 30 minutos, com rede normal, rede lenta e ligação suspensa. Os limites finais devem ser documentados em tempo e bytes, com teste determinístico de saturação.

## Privacidade e retenção

- Áudio existe apenas em RAM durante a sessão ativa e possível fallback do mesmo turno; é limpo em final, cancel, timeout, expiração ou encerramento.
- Não criar ficheiros de áudio, dumps, `.bak`, retry persistente ou anexos de diagnóstico para este modo.
- Partials não entram no histórico persistido. Apenas o final confirmado segue a política cifrada de retenção textual.
- Logs não incluem áudio, transcript, partials, prompts, keywords, API keys, payloads ou corpos remotos. IDs remotos também não são necessários em exportações de diagnóstico.
- Telemetria/diagnóstico fica limitado a códigos de estado sanitizados, durações, bytes, contagens, geração e tipo de fallback.
- Cancel e “apagar tudo” devem tornar o conteúdo inacessível pelos caminhos normais e de recuperação definidos para a Fase 6.3.
- O utilizador vê antes de ativar que áudio é enviado ao provider selecionado e que o modo pode ter custo superior ao push-to-talk atual.

## Custos e guardrails

Não se fixa um preço neste spike. Antes do benchmark deve ser guardado um snapshot datado da página oficial de preços e confirmado o modo de faturação do modelo escolhido.

Medir por variante:

- segundos de áudio capturados, enviados, repetidos e faturáveis;
- custo estimado por minuto capturado e por minuto transcrito com sucesso;
- custo adicional causado por reconnect/fallback;
- sessões vazias, canceladas e falhadas que possam ter custo;
- diferença para o provider e caminho de ficheiro atuais.

Guardrails do protótipo: opt-in; aviso de experimental/custo; uma sessão por vez; duração máxima configurada e visível; sem reconnect ilimitado; sem fallback pago automático depois de commit ambíguo. O limite de duração e a política de custo só passam a defaults de produto depois de medidos e aprovados.

## Benchmark pt-PT

### Corpus

Usar gravações consentidas e não pessoais, com transcrição de referência revista por uma pessoa. O mesmo áudio alimenta todas as variantes para tornar a comparação justa.

- Durações alvo: 30 s, 2 min, 5 min, 10 min e uma sessão contínua de 30 min.
- Português europeu de diferentes regiões/idades/velocidades, com pausas e autocorreções naturais.
- Microfones integrado, headset e USB; ambientes silencioso, ventoinha/escritório e ruído de rua controlado.
- Nomes próprios, moradas fictícias, números, datas, moedas, siglas, pontuação ditada, termos técnicos e termos do glossário.
- Code-switch pt-PT/inglês sem mudar silenciosamente a língua preferida.
- Falhas reproduzíveis: latência, perda de pacotes, offline, reconnect antes/depois do commit, sleep/resume e cancel em cada estado ativo.

Variantes mínimas:

1. upload/provider atual;
2. upload OpenAI recomendado na Fase 5;
3. Realtime `gpt-live-transcribe` nos níveis de delay que o teste preliminar selecionar;
4. Realtime committed-turn `gpt-transcribe`, se continuar oficialmente suportado.

### Métricas

| Área | Métrica | Como medir |
|---|---|---|
| Latência | primeiro partial útil p50/p95 | início da fala até partial que sobrevive no final |
| Latência | commit → final p50/p95 | commit local até final associado ao `item_id` |
| Latência | commit → paste p50/p95 | inclui finalização local e entrega |
| Qualidade | WER e CER | contra referência pt-PT, com normalização publicada |
| Qualidade | exactidão de termos/números | conjunto anotado, com e sem keywords |
| Estabilidade | churn de partials | caracteres apresentados e depois substituídos |
| Integridade | vazio/truncado/duplicado/out-of-order | contagem separada por cenário |
| Recuperação | sucesso e tempo de reconnect/fallback | antes e depois do commit |
| Recursos | pico/steady-state de RAM, CPU e fila | sessões de 10 e 30 min |
| Rede | bytes enviados e repetidos | por minuto e por reconnect |
| Custo | custo por minuto útil | preço oficial datado + uso medido |
| Privacidade | resíduos após final/cancel/purge | inspeção de app-data, logs e exports |

### Gate go/no-go

O modo só avança para implementação de produto se, no corpus pt-PT acordado:

- reduzir pelo menos 30% o p95 `commit → paste` nos ditados de 2–10 minutos face ao melhor caminho de ficheiro aplicável;
- oferecer partial útil antes do fim em pelo menos 95% dos ditados com fala válida, sem usar partial como resultado final;
- não piorar WER em mais de 1 ponto percentual absoluto nem a exactidão de termos anotados em mais de 2 pontos percentuais;
- produzir zero paste duplicado, final de sessão errada ou perda silenciosa nos testes de concorrência/falha;
- cancelar sem histórico/paste em 100% dos estados testados e completar purge sem resíduos recuperáveis pela app;
- manter memória bounded no ensaio de 30 minutos e passar saturação sem drop silencioso;
- documentar custo e obter aprovação explícita se o custo por minuto útil exceder 2× o comparador atual.

Falhar um critério de integridade ou privacidade é **no-go**. Se apenas a vantagem de latência falhar, manter o push-to-talk e arquivar o protótipo; não ativar realtime por marketing ou por existência de partials.

## Folha de resultados — por preencher

| Variante | WER | Termos | Partial p95 | Commit-final p95 | Commit-paste p95 | RAM 30 min | Custo/min útil | Integridade |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Provider atual / ficheiro | Por medir | Por medir | N/A | Por medir | Por medir | Por medir | Por medir | Por medir |
| OpenAI / ficheiro | Por medir | Por medir | N/A | Por medir | Por medir | Por medir | Por medir | Por medir |
| `gpt-live-transcribe` | Por medir | Por medir | Por medir | Por medir | Por medir | Por medir | Por medir | Por medir |
| `gpt-transcribe` committed-turn | Por medir | Por medir | Por medir | Por medir | Por medir | Por medir | Por medir | Por medir |

Decisão final: **não tomada — benchmark e smoke nativo ainda não executados**.
