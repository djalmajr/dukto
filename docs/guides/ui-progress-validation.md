# UI: progresso concorrente e configurações

Rodada nativa Mac/Windows de 14/09/2026 (logs UTC de 15/09). Builds desktop reais, menus por host e seletores nativos; sem simulação de eventos de progresso.

## Resultado observado

- Mac → Windows: arquivo de 4 GiB em progresso, seguido de envio separado de 64 KiB. A captura Mac mostra duas linhas independentes, grande em 2,7 GB e pequeno aguardando em 0 B. A captura Windows mostra o pequeno como **Recebido** enquanto o grande ainda estava em **3,3 GB de 4,0 GB**.
- Windows → Mac: arquivo de 4 GiB e arquivo de 128 KiB concluídos. O pedido de 128 KiB identificou `dj4lm@Alien`; houve interação manual do usuário antes do clique de aceitação do agente. O pequeno foi gravado às 01:42:35.816 UTC, antes do último dado do grande às 01:42:55.351 UTC. A UI continuou mostrando 3,2 GB de 4,0 GB durante essa sequência.
- Os quatro arquivos sintéticos recebidos tiveram tamanho e SHA-256 idênticos às respectivas origens. O grande usa bytes zero em arquivo esparso na origem, mas os 4 GiB atravessaram o transporte normalmente.
- Houve ações manuais do usuário durante a rodada, incluindo aceitação e outros envios. Não atribuímos esses cliques aos agentes. Arquivos pessoais adicionados durante o teste foram preservados no Windows.

## Análise das barras

Cada transferência tem linha, barra e cancelamento próprios dentro do host. Envio usa azul; recebimento usa verde. Adicionar um envio não reiniciou a barra já ativa, e o pequeno concluiu antes do grande. O estado concluído permanece aproximadamente três segundos e depois some.

Dois pontos de apresentação continuam visíveis: a linha não informa o nome do arquivo, dificultando distinguir transferências semelhantes; antes da aceitação, ela já diz **Sending**, embora esteja em 0 B. Os arquivos pequenos terminam rápido demais para garantir observação de uma barra parcialmente preenchida; a evidência forte é o estado concluído ao lado do grande ainda ativo.

## Taxa de recebimento corrigida

O receptor emitia `speed_bps: 0`, fazendo a UI omitir a taxa. Agora calcula bytes recebidos / tempo transcorrido após a aceitação, cumulativo por sessão e independente das demais transferências. UI e CLI usam o mesmo receptor. A captura real do Mac confirmou **Receiving · 62.0 MB/s**, com 1,1 GB de 4,0 GB, e depois **59.1 MB/s** com 3,2 GB de 4,0 GB.

O teste E2E QUIC foi ampliado para exigir taxa positiva no recebimento e integridade dos contadores; os oito testes de transferência passaram. Build CLI release e build desktop macOS passaram.

## Modal de configurações

O modal usa o Dialog da biblioteca do projeto com overlay escuro. Conforme pedido explícito, clicar no overlay fecha as configurações. Clicar no conteúdo mantém aberto; o X continua funcionando. Escape permanece bloqueado. O modal de recebimento mantém suas regras de aceitação/rejeição.

Mac: overlay, clique interno, clique externo, reabertura, seletor nativo de pasta e X validados. Destino `/Users/djalmajr/Shared` restaurado pela UI. TypeScript, Biome direcionado e 29 testes Bun passaram. Windows: fontes sincronizadas e TypeScript/Biome aprovados; build desktop final com taxa e overlay aprovado. Overlay visível, clique interno, clique externo, reabertura e X validados na UI real. Destino `C:\Users\dj4lm\Shared` confirmado após reabrir e cinco fixtures removidas por nome e hash. Ambos os apps permanecem abertos nos builds atualizados.

## Descoberta e limites

Após reiniciar o Mac para carregar a correção, o Windows exibiu **Peer has no IPv4 address** apesar do card presente. Reabrir o Windows recuperou a descoberta e o envio funcionou. A causa desse estado transitório ainda não foi corrigida nesta rodada. A revisão da integração também identificou que o desktop tenta apenas o primeiro IPv4 anunciado, enquanto o CLI tenta todos; redes com múltiplas interfaces continuam sendo uma limitação a validar.

A interface foi observada a 600 px de largura. Esta rodada não revalidou os limites de redimensionamento, Linux, cancelamento manual concorrente ou preenchimento intermediário dos arquivos pequenos.

## Evidências

Arquivos em `.cache/ui-concurrency/`:

- `03-small64-parallel.png`: duas transferências no mesmo host.
- `windows-small-complete-large-active.png`: pequeno concluído com grande ativo.
- `05-receive-speed.png`: taxa de recebimento de 62 MB/s.
- `06-large-after-small.png` e `.txt`: grande ainda ativo após interação manual no pedido pequeno.
- `09-settings-overlay.png` e `10-settings-restored.png`: overlay e destino original.
- `manifest.json`, `windows-manifest-bundle.json`, `windows-received-result.json`, `mac-received-result.json`: tamanhos e hashes.

Somente arquivos sintéticos conhecidos foram removidos. No Windows, os arquivos pessoais recebidos permanecem em `C:\Users\dj4lm\repo.git\djalmajr\dukto\.cache\ui-concurrency\received`; essa pasta não deve ser removida recursivamente.

## Integração: remetente direto do CLI

A revisão encontrou que um remetente ausente do mDNS aparecia somente pelo ID curto e não ganhava card de progresso. Reproduzimos o modal com `From: 7821cef1`; o usuário aceitou esse primeiro pedido, e os 37 bytes chegaram corretamente.

O header agora inclui identidade opcional do remetente, preservando compatibilidade com os headers 0.2 anteriores. O desktop cria um card transitório vinculado à transferência, sem menu de envio ou destino de drop quando não há endpoint descoberto. No novo build macOS, o CLI por endereço direto mostrou `Integration CLI@Run2Biz.local` no modal; após aceitação pelo agente, o card exibiu **Receiving · 103.2 MB/s**, em **32.1 MB / 2.0 GB**. Os 2 GiB chegaram com ACK e SHA-256 idêntico, e o card desapareceu após concluir. As duas cópias sintéticas foram removidas por nome após verificação.

Evidências locais: `.cache/integration/direct-cli/after-modal.png`, `after-progress.png`, `after-result.json` e `after.log`. A tentativa de observar o menu de outro host durante o progresso foi interrompida pela mudança de estado da UI; não é evidência de persistência desse menu. Este teste comprova CLI → desktop macOS; desktop → CLI e o novo caso direto no Windows não foram revalidados nesta rodada.
