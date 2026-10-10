import type { ReactNode } from "react";

export type SelectionOption = {
  value: string;
  label: string;
  lang?: string;
  description?: string;
  group?: string;
  keywords?: readonly string[];
  disabled?: boolean;
  icon?: ReactNode;
};
export type SelectionOptionInput = SelectionOption | readonly [string, string];
export type OptionLoader = (
  query: string,
  context: { signal: AbortSignal },
) => Promise<readonly SelectionOption[]>;

export function selectionOptions(
  options: readonly SelectionOptionInput[],
): SelectionOption[] {
  return options.map((option) =>
    Array.isArray(option)
      ? { value: option[0], label: option[1] }
      : (option as SelectionOption),
  );
}
export function normalizeSearch(value: string) {
  return value
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLocaleLowerCase()
    .trim();
}
export function filterOptions(
  options: readonly SelectionOption[],
  query: string,
) {
  const words = normalizeSearch(query).split(/\s+/).filter(Boolean);
  return options.filter((option) => {
    const text = normalizeSearch(
      [
        option.label,
        option.value,
        option.description,
        option.group,
        ...(option.keywords ?? []),
      ]
        .filter(Boolean)
        .join(" "),
    );
    return words.every((word) => text.includes(word));
  });
}
export function mergeOptions(
  ...lists: readonly (readonly SelectionOption[])[]
) {
  const map = new Map<string, SelectionOption>();
  for (const list of lists)
    for (const option of list) map.set(option.value, option);
  return [...map.values()];
}
export function toggleSelection(values: readonly string[], value: string) {
  return values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value];
}
