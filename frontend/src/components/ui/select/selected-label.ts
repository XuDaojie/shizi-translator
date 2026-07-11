export const findSelectedLabel = (
  options: readonly { value: string; label: string }[],
  value?: string,
): string | undefined => options.find((option) => option.value === value)?.label
