import { For, type JSX, Show, createSignal, onCleanup, onMount } from "solid-js";
import IconWindows from "~icons/bi/windows";
import ArrowDown from "~icons/lucide/arrow-down";
import ArrowLeftRight from "~icons/lucide/arrow-left-right";
import ArrowRight from "~icons/lucide/arrow-right";
import ArrowUpRight from "~icons/lucide/arrow-up-right";
import IconCheck from "~icons/lucide/check";
import IconCopy from "~icons/lucide/copy";
import IconExternalLink from "~icons/lucide/external-link";
import IconFolder from "~icons/lucide/folder";
import IconFolderTree from "~icons/lucide/folder-tree";
import IconMenu from "~icons/lucide/menu";
import IconNetwork from "~icons/lucide/network";
import IconShieldCheck from "~icons/lucide/shield-check";
import IconClose from "~icons/lucide/x";
import IconApple from "~icons/simple-icons/apple";
import IconGithub from "~icons/simple-icons/github";
import IconLinux from "~icons/simple-icons/linux";
import logoUrl from "../../assets/brand/dukto-icon.svg";
import { Button } from "../../src/components/ui/button";
import { Preferences } from "./Preferences";
import { type Guide, guides } from "./guides";
import { LocaleContext, localeValue, routeLocale, stripLocale, useLocale } from "./i18n";

const github = "https://github.com/djalmajr/dukto";
function Mark() {
	return <img class="brand-mark" src={logoUrl} alt="" width="44" height="44" />;
}
function Arrow() {
	return <ArrowUpRight class="arrow-icon" aria-hidden="true" />;
}
function LinkButton(props: { href: string; children: JSX.Element; secondary?: boolean }) {
	return (
		<Button
			as="a"
			href={props.href}
			variant={props.secondary ? "outline" : "default"}
			size="lg"
			class={`action ${props.secondary ? "secondary" : ""}`}
		>
			{props.children}
		</Button>
	);
}
function Header(props: { path: string }) {
	const { t, localPath } = useLocale();
	const [open, setOpen] = createSignal(false);
	return (
		<header class="header">
			<a class="brand" href={localPath("/")} aria-label={t("Dukto, início")}>
				<Mark />
				Dukto
			</a>
			<nav id="main-navigation" classList={{ "nav-open": open() }} aria-label={t("Principal")}>
				<a href={localPath("/")} aria-current={props.path === "/" ? "page" : undefined}>
					{t("Início")}
				</a>
				<a
					href={localPath("/docs/primeiros-passos/")}
					aria-current={
						props.path === "/docs" || props.path.startsWith("/docs/") ? "page" : undefined
					}
				>
					{t("Documentação")}
				</a>
				<div class="mobile-download">
					{" "}
					<LinkButton href={localPath("/downloads/")}>
						{t("Obter o Dukto")}
						<ArrowDown class="arrow-icon" aria-hidden="true" />
					</LinkButton>
				</div>
			</nav>
			<div class="header-actions">
				<Preferences />
				<Button
					as="a"
					href={`${github}/releases`}
					target="_blank"
					rel="noopener noreferrer"
					variant="ghost"
					size="icon"
					class="preference-button"
					aria-label="GitHub"
					title="GitHub"
				>
					<IconGithub aria-hidden="true" />
				</Button>
				<div class="header-download">
					{" "}
					<LinkButton href={localPath("/downloads/")}>
						{t("Obter o Dukto")}
						<ArrowDown class="arrow-icon" aria-hidden="true" />
					</LinkButton>
				</div>
			</div>
			<Button
				class="mobile-menu"
				variant="ghost"
				size="icon"
				aria-controls="main-navigation"
				aria-label={t(open() ? "Fechar navegação" : "Abrir navegação")}
				aria-expanded={open()}
				onClick={() => setOpen(!open())}
			>
				<Show when={open()} fallback={<IconMenu aria-hidden="true" />}>
					<IconClose aria-hidden="true" />
				</Show>
			</Button>
		</header>
	);
}
function Footer() {
	const { t, localPath } = useLocale();
	return (
		<footer>
			<a class="brand" href={localPath("/")} aria-label={t("Dukto, início")}>
				<Mark />
			</a>
			<p>{t("Menos caminho. Mais conexão.")}</p>
			<div>
				<a href={localPath("/docs/privacidade/")}>{t("Privacidade")}</a>
				<a
					class="author-link"
					href="https://djalmajr.dev"
					target="_blank"
					rel="noopener noreferrer"
				>
					<span>
						{t("Feito por")} {t("Djalma Jr.")}
					</span>
					<IconExternalLink class="arrow-icon" aria-hidden="true" />
				</a>
			</div>
		</footer>
	);
}
function CopyCode(props: { code: string; label?: string }) {
	const { t } = useLocale();
	const [status, setStatus] = createSignal("Copiar");
	async function copy() {
		try {
			await navigator.clipboard.writeText(t(props.code));
			setStatus("Copiado");
		} catch {
			setStatus("Selecione o texto");
		}
	}
	return (
		<div class="code-block">
			<div class="code-bar">
				<span>{props.label || "TERMINAL"}</span>
				<Button
					variant="ghost"
					size="icon"
					class="copy"
					onClick={copy}
					aria-label={t("Copiar comando")}
					title={t(status())}
				>
					<Show when={status() === "Copiado"} fallback={<IconCopy aria-hidden="true" />}>
						<IconCheck aria-hidden="true" />
					</Show>
				</Button>
			</div>
			<pre>
				<code>{t(props.code)}</code>
			</pre>
			<span class="sr-only" aria-live="polite">
				{t(status())}
			</span>
		</div>
	);
}
function TransferScene() {
	const { t } = useLocale();
	return (
		<div
			class="scene"
			role="img"
			aria-label={t(
				"Ilustração: uma pasta Projeto enviada do MacBook para um computador Windows pela rede local",
			)}
		>
			<div class="scene-top">
				{t("A MESMA REDE. NOVAS POSSIBILIDADES.")}
				<span class="scene-index">01 — 02</span>
			</div>
			<div class="orbit orbit-one" />
			<div class="orbit orbit-two" />
			<div class="device device-left">
				<div class="device-icon">
					<IconApple aria-hidden="true" />
				</div>
				<strong>MacBook</strong>
				<span>{t("Seu computador")}</span>
			</div>
			<div class="connection">
				<i />
				<i />
				<i />
				<span>
					<ArrowRight class="arrow-icon" aria-hidden="true" />
				</span>
				<i />
				<i />
			</div>
			<div class="device device-right">
				<div class="device-icon">
					<IconWindows aria-hidden="true" />
				</div>
				<strong>Desktop</strong>
				<span>{t("Logo ali")}</span>
			</div>
			<div class="parcel">
				<IconFolder class="folder-icon" aria-hidden="true" />
				<div>
					<strong>{t("Projeto de hoje")}</strong>
					<span>{t("Uma pasta. Tudo junto.")}</span>
				</div>
				<span class="parcel-check">
					<IconCheck aria-hidden="true" />
				</span>
			</div>
			<div class="scene-bottom">
				<span class="platform-flow">
					MACOS <ArrowLeftRight class="arrow-icon" aria-hidden="true" /> WINDOWS{" "}
					<ArrowLeftRight class="arrow-icon" aria-hidden="true" /> LINUX
				</span>
				<span>{t("Conexão local")}</span>
			</div>
		</div>
	);
}
function Home() {
	const { t, localPath } = useLocale();
	return (
		<>
			<main>
				<section class="hero wrap">
					<div class="hero-copy">
						<div class="eyebrow">{t("SEUS ARQUIVOS, LOGO ALI")}</div>
						<h1>
							{t("De um computador.")}
							<br />
							<em>{t("Para o outro.")}</em>
						</h1>
						<p class="lead">
							{t(
								"Envie arquivos e pastas entre Mac, Windows e Linux. Direto pela sua rede, com a simplicidade que esse caminho merece.",
							)}
						</p>
						<div class="hero-actions">
							<LinkButton href={localPath("/downloads/")}>
								{t("Obter o Dukto")}
								<ArrowDown class="arrow-icon" aria-hidden="true" />
							</LinkButton>
							<a class="text-link" href={localPath("/docs/primeiros-passos/")}>
								{t("Comece em poucos passos")}
								<ArrowRight class="arrow-icon" aria-hidden="true" />
							</a>
						</div>
						<div class="hero-note">
							<span>{t("Sem conta.")}</span>
							<span>{t("Sem upload para a nuvem.")}</span>
							<span>{t("Pela rede local.")}</span>
						</div>
					</div>
					<TransferScene />
				</section>
				<section class="platform-strip wrap" aria-label={t("Disponibilidade")}>
					<span>
						{t("COMPUTADORES DIFERENTES.")}
						<br />
						<strong>{t("A mesma conversa.")}</strong>
					</span>
					<div>
						<span>
							<IconApple aria-hidden="true" /> macOS
						</span>
						<span>
							<IconWindows aria-hidden="true" /> Windows
						</span>
						<span>
							<IconLinux aria-hidden="true" /> Linux
						</span>
					</div>
					<a href={localPath("/docs/cli/")}>
						{t("Também no terminal")}
						<Arrow />
					</a>
				</section>
				<section class="how wrap" id="como-funciona">
					<div class="section-head">
						<div>
							<span class="eyebrow">{t("MENOS ETAPAS ENTRE VOCÊ E SEUS ARQUIVOS")}</span>
							<h2>{t("Simples. Direto. Seu.")}</h2>
						</div>
						<p>
							{t("Um arquivo solto ou o projeto inteiro.")}
							<br />
							{t("O caminho é o mesmo.")}
						</p>
					</div>
					<div class="steps">
						<For
							each={[
								{
									n: "01",
									icon: <ArrowLeftRight class="arrow-icon" aria-hidden="true" />,
									title: t("Encontre seu outro computador"),
									text: t(
										"Abra o Dukto nos dois dispositivos, conectados à mesma rede. Eles se encontram por descoberta local.",
									),
								},
								{
									n: "02",
									icon: <ArrowUpRight class="arrow-icon" aria-hidden="true" />,
									title: t("Escolha o que vai junto"),
									text: t(
										"Documentos, fotos ou pastas. A CLI também reúne arquivos e várias pastas em um único envio.",
									),
								},
								{
									n: "03",
									icon: <IconCheck aria-hidden="true" />,
									title: t("Aceite. E pronto."),
									text: t(
										"Confirme o recebimento no destino. A transferência acontece diretamente entre os computadores.",
									),
								},
							]}
						>
							{(s) => (
								<article class="step">
									<div class="step-top">
										<span>{s.icon}</span>
										<small>{s.n}</small>
									</div>
									<h3>{t(s.title)}</h3>
									<p>{t(s.text || "")}</p>
								</article>
							)}
						</For>
					</div>
				</section>
				<section class="terminal-section wrap">
					<div>
						<span class="eyebrow">{t("PARA QUEM PENSA EM COMANDOS")}</span>
						<h2>
							{t("Também fala")}
							<br />
							<em>{t("terminal.")}</em>
						</h2>
						<p>
							{t(
								"A mesma base de transferência, pronta para seu fluxo de trabalho. Envie uma pasta, automatize um recebimento e leia o resultado em JSON.",
							)}
						</p>
						<a class="text-link" href={localPath("/docs/cli/")}>
							{t("Explore a CLI")}
							<ArrowRight class="arrow-icon" aria-hidden="true" />
						</a>
					</div>
					<CopyCode
						label="DUKTO CLI"
						code={
							"# Encontre seus dispositivos\ndukto peers\n\n# Arquivos e pastas, em uma viagem\ndukto send --peer ID -- ./foto.jpg ./projeto"
						}
					/>
				</section>
				<section class="principles wrap">
					<div>
						<span class="eyebrow">{t("PEQUENO POR ESCOLHA")}</span>
						<h2>
							{t("Seus arquivos seguem")}
							<br />
							{t("o caminho mais curto.")}
						</h2>
					</div>
					<div class="principle-list">
						<article>
							<span>
								<IconNetwork aria-hidden="true" />
							</span>
							<div>
								<h3>{t("De dispositivo para dispositivo")}</h3>
								<p>
									{t(
										"O conteúdo vai pela rede local, sem um serviço de armazenamento intermediário.",
									)}
								</p>
							</div>
						</article>
						<article>
							<span>
								<IconShieldCheck aria-hidden="true" />
							</span>
							<div>
								<h3>{t("Criptografia no caminho")}</h3>
								<p>
									{t(
										"QUIC e Noise protegem a sessão. Você controla a aceitação de cada transferência.",
									)}
								</p>
							</div>
						</article>
						<article>
							<span>
								<IconFolderTree aria-hidden="true" />
							</span>
							<div>
								<h3>{t("Sua organização vai junto")}</h3>
								<p>
									{t(
										"Envie pastas com sua estrutura preservada. Arquivos e subpastas chegam organizados ao destino.",
									)}
								</p>
								<a href={localPath("/docs/arquivos-e-pastas/")}>
									{t("Saiba como enviar pastas")}
									<Arrow />
								</a>
							</div>
						</article>
					</div>
				</section>
				<section class="closing wrap">
					<div>
						<span class="eyebrow">{t("VAMOS ENCURTAR ESSE CAMINHO?")}</span>
						<h2>
							{t("Seu outro computador")}
							<br />
							{t("está logo ali.")}
						</h2>
					</div>
					<div>
						<LinkButton href={localPath("/downloads/")}>
							{t("Encontre sua versão")}
							<ArrowDown class="arrow-icon" aria-hidden="true" />
						</LinkButton>
						<a href={localPath("/docs/primeiros-passos/")}>
							{t("Ou leia o guia de primeiros passos")}{" "}
							<ArrowRight class="arrow-icon" aria-hidden="true" />
						</a>
					</div>
				</section>
			</main>
			<Footer />
		</>
	);
}
function Docs(props: { guide: Guide }) {
	const { t, localPath } = useLocale();
	const [activeSection, setActiveSection] = createSignal(0);
	const sections: HTMLElement[] = [];
	onMount(() => {
		let frame = 0;
		const update = () => {
			frame = 0;
			const readingLine = Math.min(180, window.innerHeight * 0.25);
			let active = 0;
			sections.forEach((section, index) => {
				if (section.getBoundingClientRect().top <= readingLine) active = index;
			});
			if (
				window.scrollY > 0 &&
				window.scrollY + window.innerHeight >= document.documentElement.scrollHeight - 2
			)
				active = sections.length - 1;
			setActiveSection(active);
		};
		const schedule = () => {
			if (!frame) frame = requestAnimationFrame(update);
		};
		window.addEventListener("scroll", schedule, { passive: true });
		window.addEventListener("resize", schedule);
		update();
		onCleanup(() => {
			window.removeEventListener("scroll", schedule);
			window.removeEventListener("resize", schedule);
			cancelAnimationFrame(frame);
		});
	});
	const groups = [...new Set(guides.filter((g) => g.group !== "PROJETO").map((g) => g.group))];
	return (
		<>
			<main class="docs-layout wrap">
				<aside class="docs-sidebar">
					<span class="eyebrow">{t("DOCUMENTAÇÃO")}</span>
					<For each={groups}>
						{(group) => (
							<div class="nav-group">
								<span>{t(group)}</span>
								<For each={guides.filter((g) => g.group === group)}>
									{(g) => (
										<a
											href={localPath(`/docs/${g.slug}/`)}
											aria-current={props.guide.slug === g.slug ? "page" : undefined}
										>
											{t(g.title)}
										</a>
									)}
								</For>
							</div>
						)}
					</For>
				</aside>
				<article class="doc-article">
					<span class="eyebrow">{t(props.guide.group)}</span>
					<h1>{t(props.guide.title)}</h1>
					<p class="doc-summary">{t(props.guide.summary)}</p>
					<div class="doc-rule" />
					<For each={props.guide.sections}>
						{(s, i) => (
							<section
								ref={(el) => {
									sections[i()] = el;
								}}
								id={`secao-${i()}`}
							>
								<h2>{t(s.title)}</h2>
								<Show when={t(s.text || "")}>
									<p>{t(s.text || "")}</p>
								</Show>
								<Show when={s.items}>
									<ul>
										<For each={s.items}>{(item) => <li>{t(item)}</li>}</For>
									</ul>
								</Show>
								<Show when={s.code}>
									<CopyCode code={s.code || ""} />
								</Show>
							</section>
						)}
					</For>
					<div class="doc-bottom">
						<span>{t("Documentação da versão em desenvolvimento")}</span>
						<a href={`${github}/issues`} target="_blank" rel="noopener noreferrer">
							{t("Sugerir uma melhoria")}
							<IconExternalLink class="arrow-icon" aria-hidden="true" />
						</a>
					</div>
					<a
						class="next-guide"
						href={localPath(
							`/docs/${guides[(guides.findIndex((g) => g.slug === props.guide.slug) + 1) % guides.length].slug}/`,
						)}
					>
						<span>{t("CONTINUE EXPLORANDO")}</span>
						<span class="next-guide-title">
							{t(
								guides[(guides.findIndex((g) => g.slug === props.guide.slug) + 1) % guides.length]
									.title,
							)}{" "}
							<ArrowRight class="arrow-icon" aria-hidden="true" />
						</span>
					</a>
				</article>
				<aside class="toc">
					<span>{t("NESTA PÁGINA")}</span>
					<For each={props.guide.sections}>
						{(s, i) => (
							<a
								href={`#secao-${i()}`}
								aria-current={activeSection() === i() ? "location" : undefined}
							>
								{t(s.title).replace(/^\d+\. /, "")}
							</a>
						)}
					</For>
				</aside>
			</main>
			<Footer />
		</>
	);
}
function Downloads() {
	const { t, localPath } = useLocale();
	return (
		<>
			<main class="wrap downloads">
				<div class="download-intro">
					<span class="eyebrow">{t("UM DUKTO PARA CADA COMPUTADOR")}</span>
					<h1>
						{t("Escolha seu")}
						<br />
						<em>{t("ponto de partida.")}</em>
					</h1>
					<p class="lead">
						{t("Aplicativo para o dia a dia. CLI para seus scripts.")}
						<br />
						{t("A mesma conexão local entre eles.")}
					</p>
				</div>
				<div class="release-notice">
					<span class="notice-icon">
						<Arrow />
					</span>
					<div>
						<strong>{t("A primeira release está a caminho.")}</strong>
						<p>
							{t(
								"Os instaladores estão em preparação. Quando publicados, os downloads oficiais estarão no GitHub Releases.",
							)}
						</p>
					</div>
					<a href={`${github}/releases`} target="_blank" rel="noopener noreferrer">
						{t("Acompanhar releases")}
						<IconExternalLink class="arrow-icon" aria-hidden="true" />
					</a>
				</div>
				<div class="download-grid">
					<For
						each={[
							{
								name: "macOS",
								icon: IconApple,
								format: "DMG",
								arch: t("Apple Silicon e Intel"),
								text: t("Abra a imagem e leve o Dukto para Aplicativos."),
							},
							{
								name: "Windows",
								icon: IconWindows,
								format: "EXE",
								arch: t("Windows 10/11 · x64"),
								text: t("Um instalador para colocar tudo no lugar."),
							},
							{
								name: "Linux",
								icon: IconLinux,
								format: "DEB / AppImage",
								arch: t("Arquiteturas indicadas na release"),
								text: t("Instale o pacote ou execute a versão portátil."),
							},
						]}
					>
						{(p) => (
							<article class="download-card">
								<span class="download-os">
									<p.icon aria-hidden="true" />
								</span>
								<h2>{p.name}</h2>
								<span>{p.arch}</span>
								<p>{p.text}</p>
								<div class="format-row">
									<span>{p.format}</span>
									<small>{t("Em preparação")}</small>
								</div>
								<LinkButton href={localPath("/docs/instalacao/")} secondary>
									{t("Guia de instalação")}
									<Arrow />
								</LinkButton>
							</article>
						)}
					</For>
				</div>
				<section class="cli-download">
					<div>
						<span class="eyebrow">{t("PREFERE O TERMINAL?")}</span>
						<h2>{t("Uma CLI. Os mesmos caminhos.")}</h2>
						<p>
							{t(
								"Os binários da CLI terão downloads próprios nas releases. Até lá, você pode compilar a partir do código.",
							)}
						</p>
						<a class="text-link" href={localPath("/docs/cli/")}>
							{t("Comandos e exemplos")} <ArrowRight class="arrow-icon" aria-hidden="true" />
						</a>
					</div>
					<CopyCode code="cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli --release" />
				</section>
				<div class="download-footnote">
					{t("Use a mesma versão nos dois computadores. Consulte as")}{" "}
					<a href={localPath("/docs/rede-e-descoberta/")}>{t("orientações de rede")}</a> {t("e a")}{" "}
					<a href={localPath("/docs/privacidade/")}>{t("documentação de segurança")}</a>.
				</div>
			</main>
			<Footer />
		</>
	);
}
function NotFound() {
	const { t, localPath } = useLocale();
	return (
		<main class="wrap not-found">
			<span class="eyebrow">{t("404 · CAMINHO NÃO ENCONTRADO")}</span>
			<h1>
				{t("Essa página")}
				<br />
				{t("não está por aqui.")}
			</h1>
			<LinkButton href={localPath("/docs/primeiros-passos/")}>
				{t("Abrir a documentação")} <ArrowRight class="arrow-icon" aria-hidden="true" />
			</LinkButton>
		</main>
	);
}
function Page(props: { path: string }) {
	const { t } = useLocale();
	const path = props.path.replace(/\/+$/, "") || "/";
	const guide = guides.find((g) => path === `/docs/${g.slug}`);
	return (
		<>
			<a class="skip-link" href="#conteudo">
				{t("Pular para o conteúdo")}
			</a>
			<Header path={path} />
			<div id="conteudo">
				<Show
					when={path === "/"}
					fallback={
						<Show
							when={path === "/downloads"}
							fallback={
								<Show when={guide || path === "/docs"} fallback={<NotFound />}>
									<Docs guide={guide || guides[0]} />
								</Show>
							}
						>
							<Downloads />
						</Show>
					}
				>
					<Home />
				</Show>
			</div>
		</>
	);
}

export function App(props: { path: string }) {
	const locale = routeLocale(props.path);
	return (
		<LocaleContext.Provider value={localeValue(locale)}>
			<Page path={stripLocale(props.path)} />
		</LocaleContext.Provider>
	);
}
