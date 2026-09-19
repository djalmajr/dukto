export function isOutsideLanguagePicker(
	picker: Pick<HTMLElement, "contains"> | undefined,
	target: EventTarget | null,
) {
	return Boolean(picker && target && !picker.contains(target as Node));
}
