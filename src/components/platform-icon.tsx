import { Match, Switch } from "solid-js";
import CibLinux from "~icons/cib/linux";
import IcBaselineApple from "~icons/ic/baseline-apple";
import MdiMicrosoftWindows from "~icons/mdi/microsoft-windows";
import MdiMonitor from "~icons/mdi/monitor";

interface PlatformIconProps {
	platform: string;
	class?: string;
}

function PlatformIcon(props: PlatformIconProps) {
	return (
		<Switch fallback={<MdiMonitor class={props.class} />}>
			<Match when={props.platform === "macos"}>
				<IcBaselineApple class={props.class} />
			</Match>
			<Match when={props.platform === "windows"}>
				<MdiMicrosoftWindows class={props.class} />
			</Match>
			<Match when={props.platform === "linux"}>
				<CibLinux class={props.class} />
			</Match>
		</Switch>
	);
}

export default PlatformIcon;
