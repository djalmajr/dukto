export type Section = { title: string; text?: string; code?: string; items?: string[] };
export type Guide = {
	slug: string;
	group: string;
	title: string;
	summary: string;
	sections: Section[];
};
export const guides: Guide[] = [
	{
		slug: "primeiros-passos",
		group: "COMECE AQUI",
		title: "Primeiros passos",
		summary: "Dois computadores. A mesma rede. Seus arquivos no lugar certo.",
		sections: [
			{
				title: "01. Abra o Dukto nos dois computadores",
				text: "Use a mesma versão em ambos. Conecte os computadores à mesma rede Wi-Fi ou Ethernet e mantenha o Dukto aberto para que possam se encontrar.",
			},
			{
				title: "02. Escolha para onde enviar",
				text: "Os dispositivos disponíveis aparecem pela descoberta local. Selecione o computador de destino e os arquivos ou pastas que deseja compartilhar.",
			},
			{
				title: "03. Aceite no computador de destino",
				text: "Confira o remetente e os itens antes de aceitar. A transferência é concluída depois que o destino confirma o recebimento. Seus arquivos ficam na pasta de destino configurada no aplicativo.",
			},
			{
				title: "Prefere o terminal?",
				text: "Abra um receptor no primeiro computador e descubra seu identificador no segundo. A CLI compartilha o protocolo do aplicativo.",
				code: "# No computador que vai receber\ndukto receive --destination ./recebidos\n\n# No computador que vai enviar\ndukto peers\ndukto send --peer ID_DO_DESTINO -- ./foto.jpg",
			},
		],
	},
	{
		slug: "instalacao",
		group: "COMECE AQUI",
		title: "Instalação",
		summary: "Escolha o aplicativo para o dia a dia ou a CLI para seu fluxo de trabalho.",
		sections: [
			{
				title: "Disponibilidade",
				text: "A primeira distribuição está em preparação. Os instaladores serão disponibilizados no GitHub Releases após a revisão. A página de downloads indicará os arquivos e arquiteturas efetivamente publicados.",
			},
			{
				title: "macOS",
				text: "O instalador será distribuído em formato DMG. Abra a imagem e copie Dukto para Aplicativos. Escolha o arquivo correspondente a Apple Silicon ou Intel. Builds locais de avaliação podem não ter assinatura ou notarização Apple.",
			},
			{
				title: "Windows",
				text: "Execute o instalador EXE e siga as etapas. Permita acesso à rede privada quando o Windows solicitar, para descobrir dispositivos e receber arquivos. Builds de avaliação ainda não têm assinatura Authenticode.",
			},
			{
				title: "Linux",
				text: "O pacote DEB atende distribuições compatíveis com Debian/Ubuntu. O AppImage é uma alternativa portátil. Depois de baixar o AppImage, torne-o executável e abra-o.",
				code: "chmod +x Dukto*.AppImage\n./Dukto*.AppImage",
			},
			{
				title: "Usar o comando dukto",
				text: "O pacote da CLI contém dukto (dukto.exe no Windows). Extraia-o em uma pasta própria e adicione-a ao PATH. Não substitua o executável gráfico, que também usa esse nome; se houver ambiguidade, use o caminho completo da CLI. Ao compilar, o arquivo gerado continua sendo dukto-cli; copie-o para outra pasta com o nome dukto ou execute-o diretamente.",
			},
			{
				title: "Compilar a CLI",
				text: "Se você tem acesso ao repositório, instale Rust e as ferramentas de compilação da sua plataforma. Execute na raiz do repositório. A CLI não precisa de WebView.",
				code: "cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --bin dukto-cli --release\n\n# O binário estará em src-tauri/target/release/",
			},
		],
	},
	{
		slug: "arquivos-e-pastas",
		group: "USANDO O DUKTO",
		title: "Arquivos e pastas",
		summary: "De um documento a uma pasta inteira, preserve a organização do que você envia.",
		sections: [
			{
				title: "Um ou vários itens",
				text: "A CLI aceita arquivos, pastas e seleções mistas no mesmo comando. Pastas são percorridas recursivamente, incluindo subpastas e diretórios vazios.",
				code: "dukto send --peer ID -- ./arquivo.pdf\ndukto send --peer ID -- ./a.txt ./b.txt\ndukto send --peer ID -- ./projeto\ndukto send --peer ID -- ./fotos ./documentos\ndukto send --peer ID -- ./nota.txt ./fotos ./documentos",
			},
			{
				title: "Nomes e conflitos",
				text: "Arquivos existentes recebem nomes alternativos para evitar sobrescrita. Nomes inválidos são ajustados para compatibilidade entre sistemas. Links simbólicos são ignorados.",
			},
			{
				title: "Quando uma transferência falha",
				text: "Arquivos parciais podem permanecer no destino. A retomada automática ainda não foi implementada. Confira a pasta de destino e inicie um novo envio; não considere apenas a barra de progresso como confirmação de conclusão.",
			},
		],
	},
	{
		slug: "cli",
		group: "USANDO O DUKTO",
		title: "Referência da CLI",
		summary: "A mesma transferência local, com comandos que cabem nos seus scripts.",
		sections: [
			{
				title: "Descobrir dispositivos",
				code: "dukto --json peers --timeout 10",
				text: "A saída lista identificador, nome, plataforma, endereços, porta e versão do protocolo dos receptores encontrados por mDNS.",
			},
			{
				title: "Receber",
				code: "dukto --name Estúdio receive --destination ./recebidos --port 4243",
				text: "Por padrão, cada envio pede aprovação no terminal. Use uma porta diferente da interface gráfica se ambos estiverem rodando no mesmo computador.",
			},
			{
				title: "Automatizar explicitamente",
				code: "dukto --json receive --destination ./recebidos --port 4243 --accept --once",
				text: "--accept autoriza os recebimentos apenas nesta execução. --once encerra depois de uma tentativa. Sem terminal interativo, --accept é obrigatório. Use uma pasta dedicada para testes.",
			},
			{
				title: "Enviar por identificador ou endereço",
				code: "dukto send --peer ID_DO_DESTINO -- ./pasta\n\n# Se a descoberta estiver indisponível\ndukto send --address 192.168.0.16:4243 -- ./pasta",
				text: "Use endereços IPv4. --timeout define o prazo total do envio em segundos (padrão: 300). No receptor, limita cada conexão ativa, não o tempo de espera ocioso.",
			},
			{
				title: "Interpretar o resultado",
				text: "--json produz um evento JSON por linha. O evento sent inclui acknowledged: true apenas após confirmação do receptor. Código de saída 0 indica sucesso; falhas retornam valor diferente de zero. Um receptor contínuo informa receive_error e continua. --data-dir permite isolar identidades.",
			},
		],
	},
	{
		slug: "rede-e-descoberta",
		group: "ENTENDA O FLUXO",
		title: "Rede e descoberta",
		summary:
			"O Dukto encontra computadores próximos pela rede local, sem conta ou servidor de transferência.",
		sections: [
			{
				title: "Na mesma rede",
				text: "A descoberta usa mDNS na porta UDP 5353. As transferências usam QUIC na porta UDP anunciada pelo receptor (4242 por padrão). Redes de convidados, isolamento Wi-Fi, VPNs ou firewalls podem impedir que dispositivos se encontrem.",
			},
			{
				title: "Não aparece na lista?",
				items: [
					"Mantenha o receptor aberto e confirme que ambos usam o mesmo protocolo.",
					"Confirme que não estão em redes isoladas ou de convidados.",
					"Verifique a liberação de mDNS e da porta UDP do receptor no firewall.",
					"Teste --address na CLI para separar problemas de descoberta dos de transferência.",
				],
			},
			{
				title: "WSL2 e máquinas virtuais",
				text: "No WSL2, o modo espelhado permite acesso pela LAN; o firewall do Hyper-V também precisa permitir as portas. Nos testes atuais, Windows e seu próprio WSL2 transferiram por endereço, mas não se descobriram mutuamente. Uma VM Ubuntu no Multipass com rede bridge foi descoberta pelos outros ambientes.",
			},
			{
				title: "Cada ambiente tem sua identidade",
				text: "Windows e WSL2 podem compartilhar o mesmo IP, mas precisam de portas distintas e têm identificadores diferentes no Dukto. Uma VM com bridge pode receber um IP próprio da rede.",
			},
		],
	},
	{
		slug: "privacidade",
		group: "ENTENDA O FLUXO",
		title: "Privacidade e segurança",
		summary: "Os arquivos viajam direto entre os computadores. Você decide o que receber.",
		sections: [
			{
				title: "Transferência direta e criptografada",
				text: "O conteúdo é transportado por QUIC com uma sessão Noise. O fluxo LAN não envia seus arquivos para um serviço de armazenamento em nuvem.",
			},
			{
				title: "Aprovação e confiança",
				text: "A interface pede aceitação para cada transferência. Na CLI, o recebimento automático depende de --accept explícito. A versão atual não possui pareamento persistente nem uma lista de dispositivos confiáveis; nomes e IDs anunciados não devem ser tratados como prova da identidade de uma pessoa.",
			},
			{
				title: "Compatibilidade do protocolo",
				text: "O protocolo 0.2 exige confirmação de recebimento com identificador, quantidade de itens e bytes. Atualize remetente e destinatário juntos. Aplicativos antigos no protocolo 0.1 não são compatíveis com essa confirmação.",
			},
		],
	},
	{
		slug: "validacao",
		group: "PROJETO",
		title: "Compatibilidade testada",
		summary: "Resultados concretos, com os limites de cada ambiente à vista.",
		sections: [
			{
				title: "Quatro ambientes, doze direções",
				text: "A rodada de 14 de setembro de 2026 validou 60 transferências CLI entre Windows, macOS, Ubuntu 26.04 no WSL2 e Ubuntu 26.04 no Multipass. Cada direção cobriu um arquivo, múltiplos arquivos, uma pasta, múltiplas pastas e seleções mistas. Estrutura, tamanhos e SHA-256 foram comparados.",
			},
			{
				title: "Descoberta: uma limitação conhecida",
				text: "Mac e VM descobriram todos os demais. Windows e WSL descobriram Mac e VM, mas não um ao outro. A transferência direta entre Windows e WSL funcionou. Essa falha de descoberta permanece aberta.",
			},
			{
				title: "O que esses testes não comprovam",
				text: "Além da matriz CLI, testamos a UI do macOS e do Windows com um arquivo grande e outro pequeno em paralelo nas duas direções e comparamos seus hashes. A interoperabilidade UI ↔ CLI ainda está pendente. Os resultados não substituem a validação em outras distribuições, redes ou arquiteturas.",
			},
		],
	},
];
