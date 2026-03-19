import type { ValidComponent } from "solid-js";
import { splitProps } from "solid-js";

import * as TextFieldPrimitive from "@kobalte/core/text-field";
import type { PolymorphicProps } from "@kobalte/core/polymorphic";

import { cn } from "~/lib/cn";

const TextField = TextFieldPrimitive.Root;

type TextFieldInputProps<T extends ValidComponent = "input"> =
	TextFieldPrimitive.TextFieldInputProps<T> & {
		class?: string | undefined;
	};

const TextFieldInput = <T extends ValidComponent = "input">(
	props: PolymorphicProps<T, TextFieldInputProps<T>>,
) => {
	const [, rest] = splitProps(props as TextFieldInputProps, ["class"]);
	return (
		<TextFieldPrimitive.Input
			class={cn(
				"flex h-8 w-full rounded-md border border-input bg-background px-2.5 text-xs ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-default disabled:opacity-60",
				props.class,
			)}
			{...rest}
		/>
	);
};

type TextFieldLabelProps<T extends ValidComponent = "label"> =
	TextFieldPrimitive.TextFieldLabelProps<T> & {
		class?: string | undefined;
	};

const TextFieldLabel = <T extends ValidComponent = "label">(
	props: PolymorphicProps<T, TextFieldLabelProps<T>>,
) => {
	const [, rest] = splitProps(props as TextFieldLabelProps, ["class"]);
	return (
		<TextFieldPrimitive.Label
			class={cn("text-xs font-medium text-muted-foreground", props.class)}
			{...rest}
		/>
	);
};

export { TextField, TextFieldInput, TextFieldLabel };
