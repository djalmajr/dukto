# Story 1: Scaffold do app

**Origem:** `planning/mvp-lan/epic.md`

## Contexto

- **Problema:** o repositorio nao tem codigo executavel. Existe apenas docs, rules e planos. Antes de qualquer feature, o projeto precisa de um scaffold funcional com build, lint e estrutura de modulos.
- **Objetivo da story:** criar a base do projeto Duto com Tauri v2 + SolidJS + Tailwind v4 + Rust, com build funcional, lint, typecheck, estrutura de modulos Rust e scripts padrao.
- **Valor esperado:** `bun run dev` abre a janela do app, `bun run lint` e `cargo check` passam sem erro. A estrutura de modulos Rust esta pronta para receber as proximas stories.
- **Restricoes:** Bun como package manager, biome para lint/format, Tailwind v4 via `@tailwindcss/vite`, SolidJS via `vite-plugin-solid`, Tauri v2.
- **Epic relacionado:** `planning/mvp-lan/epic.md` — Story 1

## Arquivos

| Arquivo | Acao | Motivo |
| --- | --- | --- |
| `package.json` | Criar | Dependencias frontend e scripts |
| `bun.lock` | Criar (auto) | Lockfile gerado pelo bun install |
| `tsconfig.json` | Criar | Config TypeScript para SolidJS |
| `vite.config.ts` | Criar | Vite + vite-plugin-solid + @tailwindcss/vite |
| `biome.json` | Criar | Config de lint e format |
| `src/index.html` | Criar | Entrypoint HTML do Tauri webview |
| `src/index.tsx` | Criar | Entrypoint SolidJS |
| `src/App.tsx` | Criar | Componente raiz (placeholder hello world) |
| `src/styles/index.css` | Criar | CSS base com import do Tailwind |
| `src-tauri/Cargo.toml` | Criar | Dependencias Rust (tauri, tokio, serde, thiserror, tracing) |
| `src-tauri/tauri.conf.json` | Criar | Config do Tauri v2 (window, identifier, build) |
| `src-tauri/capabilities/default.json` | Criar | Permissoes default do Tauri v2 |
| `src-tauri/src/main.rs` | Criar | Entrypoint Rust minimo com setup do Tauri |
| `src-tauri/src/lib.rs` | Criar | Re-exports dos modulos |
| `src-tauri/src/commands/mod.rs` | Criar | Modulo de Tauri commands (vazio, com greet placeholder) |
| `src-tauri/src/protocol/mod.rs` | Criar | Modulo de protocolo (vazio) |
| `src-tauri/src/discovery/mod.rs` | Criar | Modulo de discovery (vazio) |
| `src-tauri/src/transfer/mod.rs` | Criar | Modulo de transfer (vazio) |
| `src-tauri/src/crypto/mod.rs` | Criar | Modulo de crypto (vazio) |
| `src-tauri/src/state/mod.rs` | Criar | Modulo de estado (vazio) |
| `src-tauri/src/platform/mod.rs` | Criar | Modulo de integracao com SO (vazio) |
| `.gitignore` | Criar | Ignorar node_modules, dist, target, .DS_Store |

## Detalhamento

### Estado atual (AS-IS)

- repositorio contem: `docs/`, `.agents/`, `planning/`, `AGENTS.md`, `CLAUDE.md`
- nenhum `package.json`, nenhum `Cargo.toml`, nenhum codigo fonte

### Estado alvo (TO-BE)

- projeto Tauri v2 funcional com SolidJS
- `bun run dev` abre janela com hello world
- modulos Rust criados e compilando
- lint e typecheck configurados e passando
- estrutura pronta para receber Story 2 (identity/state)

### Escopo

**Inclui:**
- scaffold completo Tauri v2 + SolidJS + Tailwind v4
- configuracao de biome, tsconfig, vite
- estrutura de modulos Rust (vazios com mod.rs)
- scripts padrao (dev, build, lint, test)
- .gitignore

**Nao inclui:**
- nenhuma logica de negocio
- nenhuma dependencia de networking (quinn, mdns-sd, snow)
- nenhum componente de UI alem do placeholder
- solid-ui (sera adicionado quando a UI real comecar)
- testes alem do build funcionar

### Criterios de aceite

- [ ] `bun install` completa sem erro
- [ ] `bun run dev` abre a janela do Tauri com uma pagina SolidJS visivel
- [ ] `bun run lint` (biome check) passa sem erro
- [ ] `tsc --noEmit` passa sem erro
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` compila sem erro
- [ ] Todos os modulos Rust listados existem e sao importados em `lib.rs`
- [ ] Hot reload do frontend funciona (alterar App.tsx e ver mudanca sem reiniciar)

### Dependencias e bloqueios

- **Dependencias:** nenhuma (primeira story do epic)
- **Bloqueios:** Rust e Tauri CLI precisam estar instalados no sistema. Bun precisa estar instalado.
- **Pre-requisitos de sistema:** `rustup`, `cargo`, Tauri CLI v2 (`cargo install tauri-cli`), `bun`

### Abordagem tecnica

1. Usar `bun create tauri-app` com opcoes: SolidJS, TypeScript, Bun
2. Ajustar o scaffold gerado:
   - adicionar Tailwind v4 via `@tailwindcss/vite`
   - configurar biome
   - ajustar tsconfig para SolidJS (jsx: "preserve", jsxImportSource: "solid-js")
   - ajustar vite.config.ts
3. Criar modulos Rust vazios com `mod.rs` em cada subpasta
4. Registrar os modulos em `lib.rs`
5. Ajustar `tauri.conf.json` com identifier `com.duto.app`, titulo "Duto"
6. Validar build e dev

### Riscos

- `create-tauri-app` pode gerar estrutura ligeiramente diferente da esperada — ajustar manualmente apos scaffold
- Tailwind v4 com `@tailwindcss/vite` pode precisar de config especifica para SolidJS — verificar compatibilidade
- Biome pode conflitar com formatacao do scaffold — rodar `biome check --write` apos setup

## Tarefas

### Fase 1 — Scaffold base

- [ ] Rodar `bun create tauri-app` com nome `duto`, identifier `com.duto.app`, SolidJS, TypeScript, Bun
- [ ] Mover conteudo gerado para a raiz do repositorio (se necessario)
- [ ] Validar que `bun install` completa
- [ ] Validar que `bun run dev` (ou `bunx tauri dev`) abre janela

### Fase 2 — Configuracao de tooling

- [ ] Adicionar `@tailwindcss/vite` como dependencia
- [ ] Criar `src/styles/index.css` com `@import "tailwindcss"`
- [ ] Importar `index.css` em `src/index.tsx`
- [ ] Configurar `vite.config.ts` com `vite-plugin-solid` + `@tailwindcss/vite`
- [ ] Criar `biome.json` com regras de lint e format
- [ ] Ajustar `tsconfig.json` para SolidJS
- [ ] Adicionar scripts em `package.json`: `dev`, `build`, `lint`, `test`
- [ ] Validar `bun run lint` passa

### Fase 3 — Estrutura Rust

- [ ] Criar `src-tauri/src/commands/mod.rs` com um command placeholder `greet`
- [ ] Criar `src-tauri/src/protocol/mod.rs` (vazio)
- [ ] Criar `src-tauri/src/discovery/mod.rs` (vazio)
- [ ] Criar `src-tauri/src/transfer/mod.rs` (vazio)
- [ ] Criar `src-tauri/src/crypto/mod.rs` (vazio)
- [ ] Criar `src-tauri/src/state/mod.rs` (vazio)
- [ ] Criar `src-tauri/src/platform/mod.rs` (vazio)
- [ ] Criar `src-tauri/src/lib.rs` importando todos os modulos
- [ ] Ajustar `src-tauri/src/main.rs` para usar `lib.rs` e registrar o command `greet`
- [ ] Ajustar `src-tauri/tauri.conf.json`: identifier `com.duto.app`, title "Duto"
- [ ] Validar `cargo check` compila sem erro

### Fase 4 — Fechamento

- [ ] Criar `.gitignore` com: `node_modules/`, `dist/`, `target/`, `.DS_Store`, `bun.lock`
- [ ] Rodar `bun run dev` e confirmar janela abre com hello world
- [ ] Rodar `bun run lint` e confirmar sem erros
- [ ] Rodar `tsc --noEmit` e confirmar sem erros
- [ ] Rodar `cargo check` e confirmar sem erros
- [ ] Testar hot reload: alterar texto no `App.tsx`, confirmar que a janela atualiza

## Verificacao

- [ ] `bun install` — sem erros
- [ ] `bun run dev` — janela abre com SolidJS hello world
- [ ] `bun run lint` — biome check sem erros
- [ ] `tsc --noEmit` — sem erros de tipo
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` — compila sem erro
- [ ] Hot reload funciona (alterar App.tsx e ver mudanca)
- [ ] Modulos Rust existem: `commands`, `protocol`, `discovery`, `transfer`, `crypto`, `state`, `platform`

## Proximo passo recomendado

Apos conclusao, iniciar **Story 2: Identidade de device e estado compartilhado** com `/story`.
