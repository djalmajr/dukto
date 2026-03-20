declare module "virtual:i18n" {
	export const translations: Record<
		string,
		Record<string, () => Promise<{ default: Record<string, unknown> }>>
	>;
}
