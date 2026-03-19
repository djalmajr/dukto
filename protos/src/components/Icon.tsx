interface IconProps {
	name: string;
	size?: number | string;
	class?: string;
}

function Icon(props: IconProps) {
	return (
		<iconify-icon
			icon={props.name}
			width={props.size ?? 24}
			height={props.size ?? 24}
			class={props.class}
		/>
	);
}

export default Icon;

// Extend JSX for iconify-icon custom element
declare module "solid-js" {
	namespace JSX {
		interface IntrinsicElements {
			"iconify-icon": {
				icon: string;
				width?: number | string;
				height?: number | string;
				class?: string;
				style?: any;
				inline?: boolean;
			};
		}
	}
}
