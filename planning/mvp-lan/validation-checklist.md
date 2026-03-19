# Story 9 — Validacao Cross-Platform

## Build macOS (aarch64) — COMPLETO

| Metrica | Resultado | Meta |
| --- | --- | --- |
| Binario raw | 12 MB | < 15 MB |
| App bundle | 13 MB | — |
| DMG | 4.4 MB | — |
| Memoria idle (com webview) | ~118 MB | < 30 MB (meta do PRD — nao atingida, dominada pelo WebKit) |
| cargo test | 56 testes GREEN | Todos passam |
| cargo check | 0 warnings | 0 |
| biome check | 0 erros | 0 |
| tsc --noEmit | 0 erros | 0 |
| App abre em release mode | OK | OK |
| mDNS registra e faz browse | OK | OK |
| Self-discovery filtrado | OK | OK |

## Bug encontrado e corrigido

- `tokio::spawn` no setup do Tauri nao funciona em release mode (sem runtime tokio explicito)
- **Fix:** trocado por `tauri::async_runtime::spawn`

## Checklist de validacao manual (2 devices na mesma LAN)

### Descoberta
- [ ] Abrir o Duto em Device A — app abre com UI funcional
- [ ] Abrir o Duto em Device B — app abre com UI funcional
- [ ] Device A aparece na lista de peers de B em < 5 segundos
- [ ] Device B aparece na lista de peers de A em < 5 segundos
- [ ] Fechar Device B — Device B desaparece da lista de A

### Transferencia (requer integracao futura — Tauri command de send)
- [ ] Device A arrasta arquivo para a janela — preview aparece
- [ ] Device A confirma envio — selecao de peer aparece
- [ ] Device A seleciona Device B — transferencia inicia
- [ ] Device B recebe notificacao de incoming transfer
- [ ] Device B aceita — arquivo e salvo na pasta destino
- [ ] Progresso visivel em ambos os lados
- [ ] Arquivo chega integro (conteudo identico)

### Pasta / multi-file
- [ ] Enviar pasta com subpastas — estrutura preservada no destino
- [ ] Enviar multiplos arquivos — todos chegam
- [ ] Diretorio vazio e criado no destino
- [ ] Conflito de nome — arquivo renomeado automaticamente

### Settings
- [ ] Pasta destino configuravel — clicar "Change" abre file picker
- [ ] Pasta destino persiste entre reinicializacoes

### UX
- [ ] Tema acompanha preferencia do sistema (claro/escuro)
- [ ] Empty state quando nao ha peers
- [ ] Drop zone visual feedback ao arrastar arquivo sobre a janela
- [ ] App e navegavel por teclado (tab entre elementos)

### Cross-platform especifico
- [ ] Testar mDNS discovery entre macOS e Windows
- [ ] Testar mDNS discovery entre macOS e Linux
- [ ] Validar sanitize de nomes (ex: caracteres invalidos no Windows)
- [ ] Verificar build de producao no Windows (`tauri build`)
- [ ] Verificar build de producao no Linux (`tauri build`)

## Nota sobre memoria

A meta do PRD de < 30 MB se refere ao processo inteiro. Em Tauri, o webview (WebKit no macOS, WebView2 no Windows) consome a maior parte da memoria. O processo Rust em si usa poucos MB. Isso e comportamento esperado e documentado pelo Tauri. A meta deve ser revisada para refletir a realidade de apps Tauri (tipicamente 80-150 MB com webview).

## Nota sobre transferencia end-to-end no app real

A integracao completa sender<->receiver pelo app (via Tauri command que inicia QUIC+Noise a partir de um peer descoberto) ainda precisa de um ultimo wiring:
1. Tauri command `send_to_peer` que recebe `device_id` + `paths`
2. Tauri listener no receiver que aceita conexoes QUIC no port anunciado
3. Emit de progress events para o frontend

O core funciona (provado pelos 7 testes e2e), mas o "botao Send no app" ainda nao dispara a transferencia real. Isso e trabalho de integracao, nao de Story 9.
