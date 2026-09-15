# Roteiro manual — Dukto no Windows e macOS

Objetivo: validar descoberta, seleção, aprovação, transferência e integridade usando as interfaces reais. Linux fica para uma rodada posterior com VM gráfica.

## 1. Preparar os dois computadores

1. Abra os builds atuais do Dukto no Windows e no Mac, na mesma rede. Evite executar duas cópias da interface no mesmo computador.
2. Windows: `C:\Users\dj4lm\repo.git\djalmajr\dukto\src-tauri\target\release\dukto.exe`.
3. Mac: o build preparado pela sessão remota está em `/private/tmp/dukto-ui-test/Dukto.app`.
4. Em Settings → Save files to → Change, escolha uma pasta exclusiva para testes em cada computador. Anote a pasta anterior para restaurá-la ao final. Não presuma que a mudança já foi aplicada: a preparação foi interrompida antes da confirmação.
5. Destino sugerido Windows: `C:\Users\dj4lm\repo.git\djalmajr\dukto\.cache\ui-matrix\received`. Mac: `/Users/djalmajr/Developer/djalmajr/dukto/.cache/ui-test/received`.
6. Resolva eventuais permissões de rede dos aplicativos. Confira que as portas anunciadas estão liberadas na rede de teste; as regras anteriores da CLI não garantem a liberação do executável gráfico.

## 2. Conferir descoberta

- [ ] Windows mostra o Mac (observado nesta preparação como `djalmajr / Run2Biz.local`).
- [ ] Mac mostra o Windows.
- [ ] Fechar o Dukto de um lado remove o dispositivo do outro após a atualização da descoberta.
- [ ] Reabrir o aplicativo faz o dispositivo reaparecer.

Registre o tempo aproximado. Se não aparecer, registre como falha de descoberta; não conte um envio por endereço direto como aprovação deste teste.

## 3. Seleções a testar

Fixtures Windows prontas em `C:\Users\dj4lm\repo.git\djalmajr\dukto\.cache\ui-matrix\source`.

| Caso | Abra a subpasta | Selecione estes itens, sem selecionar a subpasta do caso |
|---|---|---|
| A — um arquivo | `single` | `win-single.bin` (~2 MiB) |
| B — vários arquivos | `multiple` | Todos os quatro arquivos: inclui acentos, espaços e arquivo vazio |
| C — uma pasta | `folder` | `WinProject` (inclui subpasta e pasta vazia) |
| D — várias pastas | `folders` | `WinAlpha`, `WinBeta`, `WinEmpty` |
| E — arquivos e pastas | `mixed` | `win-loose.txt` e `WinMixed` juntos |

No Mac, use as fixtures em `/Users/djalmajr/Developer/djalmajr/dukto/.cache/ui-test/source`, verificando a estrutura antes de selecionar. Se não encontrar equivalentes, prepare os mesmos cinco conjuntos no Finder; inclua arquivo vazio, nome com acento/espaço, subpasta e pasta vazia.

## 4. Procedimento UI → UI

Repita A–E de Windows para Mac e depois de Mac para Windows.

1. Use um destino vazio por caso (por exemplo `received/win-ui_mac-ui/A`), configurando-o nas preferências do receptor. Isso evita que arquivos de tentativas anteriores mascarem erros.
2. Para A e B: abra o menu ⋮ do dispositivo de destino, escolha **Adicionar arquivos** e selecione os arquivos no diálogo nativo.
3. Para C, D e E: use **Adicionar pastas** no menu ⋮ do destino ou arraste os itens do Explorer/Finder diretamente para o card desse destino. Combine arquivos e pastas adicionando-os ao mesmo card.
4. Confira a prévia: nomes, destino, quantidade e tamanho. Uma pasta selecionada pode representar vários itens no protocolo; confira principalmente se os itens escolhidos estão corretos.
5. Confirme o envio. No receptor, confira remetente, quantidade e tamanho, então aceite.
6. Confira progresso e conclusão nos dois lados. Não deve ficar indefinidamente em “aguardando” após o recebimento.
7. Abra a pasta de destino. Verifique nomes, estrutura, arquivos vazios e pastas vazias. Abra os textos e compare tamanhos; faça a checagem de hash descrita abaixo.
8. Registre o resultado antes de passar ao próximo caso.

Para A e B, faça também uma repetição por arrastar e soltar em cada sistema, para cobrir as duas entradas da UI.

## 5. UI ↔ CLI

Repita os mesmos cinco casos nas quatro direções adicionais da matriz abaixo. Mantenha a UI no receptor quando estiver testando CLI → UI: é nela que você deve aceitar.

### Preparar o terminal

Execute a partir da raiz do repositório. Os comandos abaixo usam o binário compilado `dukto-cli`; o pacote distribuível usa o nome `dukto`.

Windows / PowerShell:

```powershell
$cli = '.\src-tauri\target\release\dukto-cli.exe'
& $cli --help
& $cli --json peers --timeout 10
```

macOS / terminal:

```sh
CLI=./src-tauri/target/release/dukto-cli
"$CLI" --help
"$CLI" --json peers --timeout 10
```

Se o binário não existir nesse caminho, use o executável CLI extraído do pacote. Não use o executável gráfico `dukto.exe` como CLI.

### UI → CLI

1. No receptor, inicie a CLI em porta diferente da UI. Use 44242, já empregada nos testes anteriores, desde que esteja livre.
2. Troque a pasta de destino por uma exclusiva para direção/caso.
3. A CLI aparecerá como `Windows-CLI-manual` ou `Mac-CLI-manual`. Se a UI também estiver aberta, os dois dispositivos virtuais podem aparecer; selecione a CLI correta.
4. Envie pela UI, aprove no terminal quando solicitado e confira a conclusão e os arquivos. `--once` encerra o receptor depois de uma tentativa; reinicie para cada caso.

Windows receptor:

```powershell
& $cli --name Windows-CLI-manual receive --port 44242 --destination .\.cache\ui-matrix\received\mac-ui_win-cli\A --once
```

Mac receptor:

```sh
"$CLI" --name Mac-CLI-manual receive --port 44242 --destination ./.cache/ui-test/received/win-ui_mac-cli/A --once
```

### CLI → UI

1. Deixe o receptor gráfico aberto e configure seu destino para a direção/caso atual.
2. Liste dispositivos com `peers` e copie o ID da UI, não o ID de outro receptor CLI.
3. Envie os caminhos do caso. Substitua `ID_DA_UI` pelo ID real e os caminhos pelos itens escolhidos. Vários arquivos/pastas são argumentos separados.
4. Aceite no aplicativo receptor e confira sua conclusão. A CLI deve terminar com código 0 e evento `sent` com `acknowledged: true`.

Windows remetente (caso A):

```powershell
& $cli --json send --peer ID_DA_UI -- .\.cache\ui-matrix\source\single\win-single.bin
$LASTEXITCODE
```

Mac remetente (substitua o caminho pelo arquivo real da fixture):

```sh
"$CLI" --json send --peer ID_DA_UI -- "/caminho/do/arquivo"
echo $?
```

Se precisar diagnosticar descoberta separadamente, substitua `--peer ID_DA_UI` por `--address IP:PORTA`, usando a porta anunciada pelo receptor. A UI preparada usa 4242; confirme o anúncio atual. Não use `--peer` e `--address` juntos.

## 6. Matriz de execução

Preencha cada célula com OK, FALHOU ou BLOQUEADO. Não marque como OK um caso que o controle da UI não permitiu executar.

| Origem → destino | A | B | C | D | E |
|---|---|---|---|---|---|
| Windows UI → Mac UI | | | | | |
| Mac UI → Windows UI | | | | | |
| Windows UI → Mac CLI | | | | | |
| Mac CLI → Windows UI | | | | | |
| Mac UI → Windows CLI | | | | | |
| Windows CLI → Mac UI | | | | | |

São 30 transferências principais, mais as repetições por arrastar e soltar dos casos A/B.

## 7. Integridade e comportamento

Compare SHA-256 de cada arquivo na origem e no destino. Um hash igual confirma conteúdo idêntico; a estrutura e as pastas vazias precisam ser conferidas separadamente.

Windows:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath 'C:\caminho\arquivo'
```

Mac:

```sh
shasum -a 256 '/caminho/arquivo'
```

O manifesto das fixtures Windows já está em `.cache/ui-matrix/windows-source-manifest.json`, com caminho, tamanho e SHA-256.

Teste também uma vez em cada direção UI → UI:

- [ ] Cancelar o seletor não inicia transferência.
- [ ] Remover um item da prévia exclui apenas esse item do envio.
- [ ] Cancelar a prévia não envia nada.
- [ ] Rejeitar no receptor informa a rejeição no remetente e não entrega os arquivos.
- [ ] Cancelar um arquivo grande durante o envio encerra a operação sem anunciar sucesso. Registre se sobraram arquivos parciais.
- [ ] Fechar o receptor durante um envio produz erro recuperável; após reabrir, um novo envio funciona.

Para os dois últimos testes, use uma cópia descartável de arquivo suficientemente grande para permitir a ação; os arquivos pequenos podem terminar antes do clique.

## 8. Registrar falhas e encerrar

Para cada falha, anote: direção, caso, nomes dos dispositivos, caminho de origem/destino, mensagem exata, comportamento esperado/observado e screenshot dos dois lados. Indique se falhou na descoberta, seleção, prévia, aprovação, transmissão ou integridade.

Ao terminar, pare receptores CLI com Ctrl+C, restaure a pasta de recebimento original nas UIs e guarde as evidências. Não apague arquivos pessoais nem resultados antes de revisar.

Os resultados da execução ficam separados em `.cache/ui-matrix/RESULTADOS.md`. Os 60 testes CLI anteriores são evidências separadas.

## 9. Adicionar itens um por vez

Repita no Windows e no Mac, enviando ao outro computador. Use uma pasta de destino exclusiva para cada execução.

1. Abra o menu ⋮ do destinatário, clique em **Adicionar arquivos** e escolha um arquivo.
2. No menu ⋮ do mesmo host, clique novamente em **Adicionar arquivos** e escolha outro arquivo. Os dois devem permanecer e o destinatário deve ser o mesmo.
3. No menu ⋮ do mesmo host, clique em **Adicionar pastas** e escolha uma pasta com subpasta e pasta vazia. Confira os três itens e a soma dos tamanhos.
4. Adicione novamente o primeiro arquivo: ele deve continuar aparecendo apenas uma vez.
5. Abra cada seletor e cancele. A seleção anterior deve permanecer intacta.
6. Remova um item e adicione-o novamente. Confira nome, quantidade e tamanho.
7. Envie, aceite no outro computador e compare nomes, estrutura, pastas vazias, tamanhos e SHA-256.
8. Repita com arrastes separados: primeiro um arquivo, depois outro, depois uma pasta. Solte sempre sobre o card do destinatário, inclusive com a prévia aberta. Nenhum arraste deve substituir os itens anteriores. Soltar no espaço vazio não deve selecionar arquivos nem escolher um host automaticamente.
9. Misture os métodos: arraste um arquivo e adicione uma pasta pelo botão; depois faça o inverso.
10. Cancele a prévia e inicie uma seleção nova. Nenhum item antigo deve reaparecer. Remova também o último item para confirmar que a seleção é encerrada.

Na altura mínima da janela, confira rolagem, botões de adicionar, remover, enviar e cancelar. Registre cada método separadamente: um teste pelo botão não comprova o arraste nativo.

## 10. Associação ao host e limites da janela

- [ ] Com dois hosts, adicione arquivos diferentes pelo menu de cada um. Cada prévia contém somente seus próprios arquivos.
- [ ] Cancele/remova itens de um host: a seleção do outro permanece intacta.
- [ ] Envie uma prévia: o pedido chega apenas ao host correspondente.
- [ ] Remova um host da descoberta durante a seleção: nenhum item migra para outro host nem reaparece quando o host retorna.
- [ ] Arraste sobre cada card: somente o destinatário sob o ponteiro fica realçado. Solte fora dos cards: nenhuma seleção muda.
- [ ] Redimensione entre 480 e 640 px; tente ultrapassar os dois limites. Confira menu, nomes longos e prévia sem rolagem horizontal.
- [ ] Na altura mínima de 360 px, alcance todos os cards e botões por rolagem vertical e teclado.

## 11. Transferências simultâneas

Use arquivos sintéticos grandes (512 MiB ou maiores) e uma pasta exclusiva de
recebimento. A conclusão dos testes CLI não substitui esta rodada nativa da UI.

- [ ] Envie um arquivo grande ao host A. Espere progresso real acima de zero e abaixo de 100%.
- [ ] Ainda durante esse envio, use o menu ⋮ do mesmo host e envie outro arquivo grande e, em uma terceira transferência, um arquivo pequeno.
- [ ] Confira três progressos independentes; o pequeno pode concluir antes do primeiro grande.
- [ ] Durante os envios, inicie um envio no sentido inverso e aceite na UI. Envio e recebimento devem progredir juntos no card correspondente.
- [ ] Faça dois pedidos de recebimento antes de responder. Aceitar/rejeitar o primeiro deve mostrar o segundo sem perder nenhum pedido.
- [ ] Cancele somente uma transferência ativa. Ela deve parar de transmitir; as demais continuam e concluem com SHA-256 correto.
- [ ] Interrompa um remetente. Apenas o recebimento correspondente deve apresentar erro; o receptor permanece disponível.
- [ ] Repita com nomes de arquivos iguais e conteúdos diferentes. Os arquivos completos devem permanecer distintos, usando sufixos de conflito.
- [ ] Confira cada recibo, quantidade, tamanho e SHA-256. Registre intervalos de progresso sobrepostos; iniciar dois processos ou ver duas barras não comprova concorrência.

Restaure as pastas originais e remova somente os arquivos sintéticos criados para
essa rodada, depois de guardar manifestos e resultados.
