import * as DropdownMenuPrimitive from "@kobalte/core/dropdown-menu";
import type { PolymorphicProps } from "@kobalte/core/polymorphic";
import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import { cn } from "~/utils/cn";

const DropdownMenu = DropdownMenuPrimitive.Root;
const DropdownMenuTrigger = DropdownMenuPrimitive.Trigger;
const DropdownMenuPortal = DropdownMenuPrimitive.Portal;

type DropdownMenuContentProps = DropdownMenuPrimitive.DropdownMenuContentProps<"div"> & {
	class?: string | undefined;
};

const DropdownMenuContent = (props: PolymorphicProps<"div", DropdownMenuContentProps>) => {
	const [local, rest] = splitProps(props, ["class"]);
	return (
		<DropdownMenuPrimitive.Content
			class={cn(
				"z-50 min-w-[8rem] overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-md outline-none data-expanded:animate-in data-closed:animate-out data-[closed]:fade-out-0 data-[expanded]:fade-in-0 data-[closed]:zoom-out-95 data-[expanded]:zoom-in-95",
				local.class,
			)}
			{...rest}
		/>
	);
};

type DropdownMenuItemProps = DropdownMenuPrimitive.DropdownMenuItemProps<"div"> & {
	class?: string | undefined;
	children?: JSX.Element;
};

const DropdownMenuItem = (props: PolymorphicProps<"div", DropdownMenuItemProps>) => {
	const [local, rest] = splitProps(props, ["class", "children"]);
	return (
		<DropdownMenuPrimitive.Item
			class={cn(
				"relative flex w-full cursor-default select-none items-center gap-2 rounded-sm px-2 py-1.5 text-xs outline-none transition-colors data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground",
				local.class,
			)}
			{...rest}
		>
			{local.children}
		</DropdownMenuPrimitive.Item>
	);
};

export {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuPortal,
	DropdownMenuTrigger,
};
