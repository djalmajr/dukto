import type { PolymorphicProps } from "@kobalte/core/polymorphic";
import * as TabsPrimitive from "@kobalte/core/tabs";
import type { Component, ComponentProps, ValidComponent } from "solid-js";
import { splitProps } from "solid-js";
import { cn } from "~/utils/cn";

const Tabs = TabsPrimitive.Root;

type TabsListProps<T extends ValidComponent = "div"> = TabsPrimitive.TabsListProps<T> & {
	class?: string | undefined;
};

const TabsList = <T extends ValidComponent = "div">(
	props: PolymorphicProps<T, TabsListProps<T>>,
) => {
	const [, rest] = splitProps(props as TabsListProps, ["class"]);
	return (
		<TabsPrimitive.List
			class={cn(
				"inline-flex h-9 w-full items-center justify-center rounded-lg bg-muted p-1 text-muted-foreground",
				props.class,
			)}
			{...rest}
		/>
	);
};

type TabsTriggerProps<T extends ValidComponent = "button"> = TabsPrimitive.TabsTriggerProps<T> & {
	class?: string | undefined;
};

const TabsTrigger = <T extends ValidComponent = "button">(
	props: PolymorphicProps<T, TabsTriggerProps<T>>,
) => {
	const [, rest] = splitProps(props as TabsTriggerProps, ["class"]);
	return (
		<TabsPrimitive.Trigger
			class={cn(
				"inline-flex flex-1 items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium ring-offset-background transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 data-[selected]:bg-background data-[selected]:text-foreground data-[selected]:shadow",
				props.class,
			)}
			{...rest}
		/>
	);
};

type TabsContentProps<T extends ValidComponent = "div"> = TabsPrimitive.TabsContentProps<T> & {
	class?: string | undefined;
};

const TabsContent = <T extends ValidComponent = "div">(
	props: PolymorphicProps<T, TabsContentProps<T>>,
) => {
	const [, rest] = splitProps(props as TabsContentProps, ["class"]);
	return (
		<TabsPrimitive.Content
			class={cn(
				"mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
				props.class,
			)}
			{...rest}
		/>
	);
};

const TabsIndicator: Component<ComponentProps<"div">> = (props) => {
	const [, rest] = splitProps(props, ["class"]);
	return (
		<TabsPrimitive.Indicator
			class={cn(
				"absolute transition-all duration-250 data-[orientation=horizontal]:bottom-[-1px] data-[orientation=horizontal]:h-[2px]",
				props.class,
			)}
			{...rest}
		/>
	);
};

export { Tabs, TabsList, TabsTrigger, TabsContent, TabsIndicator };
