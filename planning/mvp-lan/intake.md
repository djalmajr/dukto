# Intake: MVP LAN Desktop

**Origem:** conversa de planejamento do produto Dukto. PRD v2.1 já existe com stack, arquitetura e funcionalidades definidas. Este intake formaliza a entrega do v0.1 como unidade de trabalho.

## Contexto

- **Problema/oportunidade:** nenhuma ferramenta ativa combina descoberta LAN zero-config, transferência segura (E2E), UX simples e multiplataforma desktop. Dukto e NitroShare morreram. LocalSend não faz internet. croc e CLI-only. Este MVP valida a tese central do produto na LAN antes de expandir para internet e mobile.

- **Objetivo inicial:** entregar um app desktop funcional que descobre peers na LAN automaticamente, transfere arquivos e pastas com criptografia E2E, e oferece UX minimalista de drag-and-drop com feedback claro de progresso e estado.

- **Sinal de valor esperado:** o usuário abre o app em 2 dispositivos na mesma rede, ve os peers aparecerem, arrasta um arquivo/pasta, e a transferência acontece com progresso visível e integridade garantida. Tempo da primeira transferência < 30 segundos.

- **Restrições e premissas:**
  - stack definida: `Tauri v2 + Rust + SolidJS + solid-ui + Tailwind v4`
  - baseline de networking: `mDNS + QUIC (quinn) + Noise (snow)`
  - projeto solo, com uso intensivo de IA na implementação
  - sem prazo rígido, mas Q2 2026 como referência do roadmap
  - validação em macOS + Windows ou Linux (via VM)
  - sem CI/CD obrigatório no v0.1, mas verificação local (lint, typecheck, cargo check, cargo test)

## Escopo inicial

### Inclui (v0.1)

- scaffold do app Tauri v2 + SolidJS
- identidade de device (`device_id`, `display_name`, fingerprint)
- descoberta de peers via mDNS
- sessão segura via QUIC + Noise handshake
- transferência de arquivo único
- transferência de múltiplos arquivos
- transferência de pasta com preservação de estrutura
- drag-and-drop
- progresso, velocidade e estado final
- accept/reject de incoming transfers
- pasta destino configurável
- notificações nativas
- regras operacionais do receiver (rename/overwrite/skip, path traversal, nomes inválidos, dirs vazios)
- tema claro/escuro (seguir sistema)

### Não inclui (v0.1)

- internet/P2P, relay, signaling
- QR code, código de pareamento
- histórico persistente
- clipboard sync, screenshot send
- favoritos, auto-accept, grupos, trusted devices
- headless/CLI mode
- CI/CD pipeline
- mobile (Android/iOS)

## Entradas e referências

- **Stakeholders:** Djalma Jr (autor, usuário primário)
- **Documentos/links:**
  - `docs/PRD.md` (v2.1) — produto, stack, arquitetura, roadmap
  - `.agents/rules/architecture.md` — regras de fronteira UI/Rust
  - `.agents/rules/protocol.md` — protocolo, framing, trust, metadata
  - `.agents/rules/frontend.md` — convenções SolidJS e UX
  - `.agents/rules/build.md` — toolchain e verificação
  - `.agents/plans/1773846250998-proud-garden.md` — plano de execução atual
- **Contexto técnico conhecido:**
  - `~/Developer/github/dukto-qt5` — referência de UX e discovery LAN
  - `~/Developer/github/nitroshare-desktop` — referência de protocolo e modelagem
  - `~/Developer/zomme/platform/apps/skedly` — referência de convenções SolidJS

## Perguntas em aberto

- [x] Stack do frontend: SolidJS + solid-ui (decidido)
- [x] Protocolo LAN: mDNS + QUIC + Noise (decidido)
- [x] Licenca: BSL ou FSL (gratuito para uso nao-comercial, codigo visivel, vira open-source apos periodo)
- [x] Tamanho de arquivo: sem limite de tamanho no envio. Testar com pelo menos 1-2 GB no v0.1
- [x] Compressao zstd: fora do v0.1. Entra depois como opcao por sessao/tipo de arquivo
- [x] Trust model v0.1: accept/reject simples (nivel 1). Canal criptografado via Noise, mas sem verificacao visual de fingerprint. Fingerprint visual (nivel 2) entra como melhoria posterior
- [x] Nome do produto: **Duto**. Inspirado no Dukto original (italiano *dotto/dutto* = duto/conduto). Curto, 4 letras, unico como marca, funciona em pt/es/it
- [ ] Licenca final exata: BSL ou FSL? Definir parametros (periodo de conversao, licenca alvo). Projeto sera privado inicialmente
- [x] Modelo de monetizacao: relay as a service (gratuito com limite + pago sem limite) como modelo principal. Licenca comercial para empresas a partir do v1.0. Sem cobranca ate v0.2+

## Próximo passo recomendado

**`epic`** — o MVP LAN é grande o suficiente para exigir decomposição em stories com dependências claras. A ordem de implementação já está desenhada no PRD (seção 11.4) e no plano atual. O próximo artefato deve ser um epic que:

1. quebre o v0.1 em stories verticais executáveis
2. defina dependências entre elas
3. estabeleca critérios de aceite por story
4. crie um roadmap interno do epic

Alternativa: se preferir ir mais rápido, pode pular direto para stories individuais seguindo a sequência do PRD 11.4, mas perde a visão consolidada de dependências.

## Verificação

- [x] O problema está claro o bastante para o próximo passo
- [x] Restrições e premissas foram explicitadas
- [x] O próximo artefato do fluxo foi definido
