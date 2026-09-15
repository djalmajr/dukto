# Validação de transferências simultâneas

Executada em 14/09/2026 (America/Maceio; logs UTC de 15/09), com CLI real, QUIC e Noise. Nenhum mock de transporte ou alteração artificial de velocidade.

## Problema reproduzido e correções

O receptor CLI aguardava uma transferência terminar antes de atender outra. Com 512 MiB, a primeira concluiu em 6,653 s; o segundo arquivo só progrediu em 7,834 s e o pequeno só concluiu em 6,833 s. A integridade estava correta, mas os recebimentos eram serializados.

Agora há uma tarefa por conexão. Aprovações interativas mantêm cada resposta associada ao respectivo ID. Arquivos de mesmo nome são reservados atomicamente, sem truncar outro recebimento. O desktop mantém uma fila de solicitações, registra o envio antes dos eventos de progresso e cancela a conexão da transferência selecionada. Eventos tardios não recriam uma transferência cancelada nem revertem estados terminais.

## LAN Mac/Windows

Duas rodadas, invertendo o iniciador. Cada rodada enviou dois arquivos de 512 MiB e um de 65.537 bytes para o mesmo receptor, além de outro arquivo de 512 MiB no sentido inverso. Os envios adicionais começaram somente após progresso real acima de zero e abaixo de 100%.

| Rodada | Segundo envio progride | Pequeno conclui | Recebimento inverso progride | Primeiro envio conclui |
| --- | --- | --- | --- | --- |
| Mac inicia → Windows | 2.049 s | 1.139 s | 2.080 s | 14.222 s |
| Windows inicia → Mac | 2.181 s | 1.353 s | 2.522 s | 21.167 s |

Tempos relativos ao início do harness no iniciador de cada rodada; não dependem da sincronização dos relógios entre máquinas. Os reports dos receptores também confirmam sobreposição. Oito transferências concluídas com recibos e SHA-256 idênticos aos manifestos de origem.

## Outras verificações

- Regressão local com arquivos de 512 MiB: segundo recebimento progride em 3,787 s, pequeno conclui em 2,788 s e fluxo inverso progride em 3,800 s, antes da primeira conclusão em 8,491 s.
- Interromper um remetente durante progresso não interrompe outro envio, preserva o arquivo concluído e mantém o receptor disponível.
- Terminal interativo real: dois pedidos pendentes; aceitar o primeiro e rejeitar o segundo grava somente o arquivo aprovado, sem confundir respostas.
- Duas conexões QUIC reais no teste de cancelamento desktop: cancelar uma mantém a outra utilizável.
- Reserva simultânea do mesmo nome preserva os dois conteúdos. Mutações que truncavam o mesmo caminho ou cancelavam todas as transferências fizeram os testes falhar; implementações corretas restauradas e testes aprovados.
- Rust sem desktop: 54 testes aprovados. Cancelamento desktop: 2 testes focados aprovados. Bun: 29 testes, 101 asserções. TypeScript e Biome dos arquivos alterados aprovados.
- Regressão CLI de arquivo único, múltiplos arquivos, pasta, múltiplas pastas e entradas inválidas aprovada. Build CLI release e bundle desktop macOS aprovados.
- O lint global continua com erros preexistentes em arquivos fora deste lote; o check direcionado passou.

## Evidências e reprodução

Execute `scripts/test-cli-concurrency.mjs` conforme [guia CLI](cli.md). O script gera e remove seus próprios arquivos temporários; `DUKTO_TEST_REPORT` conserva a linha do tempo em JSON.

Nesta execução, `.cache/concurrency-test/` guarda os reports locais `before.json` e `after.json`, manifestos Mac/Windows, logs de aprovação, reports `mac-wave1/report.json`, `mac-wave2/report.json`, `windows-wave1-result.json` e `windows-wave2-result.json`. Arquivos grandes sintéticos são removidos após a validação, mantendo logs e manifestos.

## Limites desta rodada

A concorrência foi validada via CLI em ambas as máquinas. As correções do desktop foram compiladas e seus contratos testados; a reprodução manual das transferências na UI permanece pendente, conforme a sequência CLI primeiro. O roteiro está em [testes manuais de UI](manual-ui-tests.md#11-transferências-simultâneas). A sessão gráfica aberta não foi reiniciada nesta rodada. A rodada nativa posterior está registrada em [validação da UI](ui-progress-validation.md).

Cancelar ou interromper um recebimento pode deixar um arquivo parcial no destino; não há retomada automática. Pastas com a mesma raiz podem compartilhar o diretório, com conflitos de arquivos resolvidos individualmente. No CLI, cada processo de envio pode ser interrompido separadamente; interromper um receptor contínuo encerra todas as conexões dele.
