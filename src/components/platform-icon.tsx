import { Match, Switch } from "solid-js";
import CarbonApplication from "~icons/carbon/application";
import CarbonDevices from "~icons/carbon/devices";
import CarbonDirectLink from "~icons/carbon/direct-link";
import CarbonLinux from "~icons/carbon/linux";
import CarbonMac from "~icons/carbon/mac";

interface PlatformIconProps {
	platform: string;
	class?: string;
}

function PlatformIcon(props: PlatformIconProps) {
	return (
		<Switch fallback={<CarbonDevices class={props.class} />}>
			<Match when={props.platform === "macos" || props.platform === "ios"}>
				<CarbonMac class={props.class} />
			</Match>
			<Match when={props.platform === "windows"}>
				<CarbonApplication class={props.class} />
			</Match>
			<Match when={props.platform === "linux"}>
				<CarbonLinux class={props.class} />
			</Match>
			<Match when={props.platform === "internet"}>
				<CarbonDirectLink class={props.class} />
			</Match>
		</Switch>
	);
}

export default PlatformIcon;
