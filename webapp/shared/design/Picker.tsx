import { useEffect, useId, useLayoutEffect, useRef, useState, type AriaAttributes, type CSSProperties, type KeyboardEvent } from 'react';
import { Check, ChevronDown, Search, X } from 'lucide-react';
import { Spinner } from './Spinner';
import { FieldFeedback } from './FieldFeedback';
import { filterOptions, mergeOptions, selectionOptions, toggleSelection, type OptionLoader, type SelectionOption, type SelectionOptionInput } from './selection-model';
import { pickerMessages } from './picker-messages';
function describedBy(id: string, hint?: string, error?: string) { return error ? `${id}-error` : hint ? `${id}-hint` : undefined; }
type PickerCommon = AriaAttributes & {
  locale: "en" | "zh" | "ja";
  label?: string;
  id?: string;
  name?: string;
  options?: readonly SelectionOptionInput[];
  selectedOptions?: readonly SelectionOption[];
  loadOptions?: OptionLoader;
  debounceMs?: number;
  minQueryLength?: number;
  searchable?: boolean;
  clearable?: boolean;
  placeholder?: string;
  hint?: string;
  error?: string;
  reserveMessage?: boolean;
  disabled?: boolean;
  required?: boolean;
  className?: string;
  maxSelected?: number;
};
export type SinglePickerProps = PickerCommon & {
  value: string;
  onChange: (value: string, option?: SelectionOption) => void;
};
export type MultiPickerProps = PickerCommon & {
  value: readonly string[];
  onChange: (value: string[], options: SelectionOption[]) => void;
};

export function Picker(
  props:
    | (SinglePickerProps & { multiple: false })
    | (MultiPickerProps & { multiple: true }),
) {
  const t = (key: keyof typeof pickerMessages, values: Record<string, string | number> = {}) => {
    const message = pickerMessages[key][props.locale === "zh" ? 0 : props.locale === "ja" ? 1 : 2];
    return message.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? `{${name}}`));
  };
  const {
    label,
    name,
    hint,
    disabled,
    required,
    className = "",
    clearable = false,
    searchable = false,
    multiple,
    loadOptions,
    debounceMs = 250,
    minQueryLength = 0,
    maxSelected,
    reserveMessage = required || Object.hasOwn(props, "error"),
    selectedOptions = [],
  } = props;
  const [invalid, setInvalid] = useState(false);
  const hasValue = props.multiple ? props.value.length > 0 : !!props.value;
  const error =
    props.error ??
    (invalid && required && !hasValue
      ? t("selection.requiredError")
      : undefined);
  const uid = useId(),
    id = props.id ?? uid,
    listId = `${id}-list`,
    root = useRef<HTMLDivElement>(null),
    anchor = useRef<HTMLDivElement>(null),
    popup = useRef<HTMLDivElement>(null),
    control = useRef<HTMLInputElement & HTMLButtonElement>(null);
  const [open, setOpen] = useState(false),
    [query, setQuery] = useState(""),
    [active, setActive] = useState(""),
    [retry, setRetry] = useState(0),
    [composing, setComposing] = useState(false);
  const [memory, setMemory] = useState<SelectionOption[]>([]);
  const [remote, setRemote] = useState<{
    query: string;
    status: "idle" | "loading" | "ready" | "error";
    options: readonly SelectionOption[];
  }>({ query: "", status: "idle", options: [] });
  const options = selectionOptions(props.options ?? []);
  const values: readonly string[] = multiple
    ? props.value
    : props.value || options.some((option) => option.value === "")
      ? [props.value]
      : [];
  const known = mergeOptions(memory, options, remote.options, selectedOptions);
  const selected = values.map(
    (value) =>
      known.find((option) => option.value === value) ?? { value, label: value },
  );
  const enough = query.trim().length >= minQueryLength;
  const waiting =
    !!loadOptions &&
    enough &&
    (composing ||
      remote.query !== query ||
      remote.status === "idle" ||
      remote.status === "loading");
  const failed =
    !!loadOptions && remote.query === query && remote.status === "error";
  const candidates = loadOptions
    ? enough && !waiting && !failed
      ? remote.options
      : []
    : searchable
      ? filterOptions(options, query)
      : options;
  const groups = new Map<string, SelectionOption[]>();
  for (const option of candidates) {
    const group = option.group ?? "";
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group)!.push(option);
  }
  const visible = [...groups.values()].flat();
  const atLimit =
    multiple && maxSelected !== undefined && values.length >= maxSelected;
  const unavailable = (option: SelectionOption) =>
    !!option.disabled || (atLimit && !values.includes(option.value));
  const enabled = visible.filter((option) => !unavailable(option));
  const activeOption =
    enabled.find((option) => option.value === active) ??
    enabled.find((option) => values.includes(option.value)) ??
    enabled[0];
  const activeId =
    open && !disabled && activeOption
      ? `${id}-option-${visible.indexOf(activeOption)}`
      : undefined;
  const typeahead = useRef({ text: "", time: 0 });

  useEffect(() => {
    if (!loadOptions || !open || disabled || !enough || composing) return;
    const controller = new AbortController();
    const timer = setTimeout(
      () => {
        setRemote({ query, status: "loading", options: [] });
        Promise.resolve()
          .then(() => loadOptions(query.trim(), { signal: controller.signal }))
          .then(
            (result) => {
              if (!controller.signal.aborted)
                setRemote({
                  query,
                  status: "ready",
                  options: mergeOptions(result),
                });
            },
            () => {
              if (!controller.signal.aborted)
                setRemote({ query, status: "error", options: [] });
            },
          );
      },
      Math.max(0, debounceMs),
    );
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [
    query,
    open,
    loadOptions,
    debounceMs,
    disabled,
    enough,
    composing,
    retry,
  ]);

  // A native popover supplies the top layer while all option UI is drawn here.
  // Keeping it in this DOM subtree also makes it work inside a modal dialog.
  useLayoutEffect(() => {
    const panel = popup.current,
      target = anchor.current;
    if (!panel || !target || !open || disabled) return;
    const place = () => {
      const r = target.getBoundingClientRect(),
        viewport = window.visualViewport;
      const vw = viewport?.width ?? innerWidth,
        vh = viewport?.height ?? innerHeight;
      const ox = viewport?.offsetLeft ?? 0,
        oy = viewport?.offsetTop ?? 0;
      const width = Math.min(Math.max(r.width, 260), vw - 16);
      const below = oy + vh - r.bottom - 8,
        above = r.top - oy - 8;
      const upward = below < 220 && above > below,
        room = Math.max(80, (upward ? above : below) - 8);
      panel.style.width = `${width}px`;
      panel.style.maxHeight = `${room}px`;
      panel.style.setProperty("--picker-room", `${Math.max(60, room - 72)}px`);
      panel.style.left = `${Math.max(ox + 8, Math.min(r.left, ox + vw - width - 8))}px`;
      panel.style.top = `${upward ? Math.max(oy + 8, r.top - 6 - Math.min(panel.scrollHeight, room)) : r.bottom + 6}px`;
    };
    panel.showPopover();
    place();
    const observer = new ResizeObserver(place);
    observer.observe(target);
    observer.observe(panel);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    window.visualViewport?.addEventListener("resize", place);
    window.visualViewport?.addEventListener("scroll", place);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      window.visualViewport?.removeEventListener("resize", place);
      window.visualViewport?.removeEventListener("scroll", place);
      if (panel.matches(":popover-open")) panel.hidePopover();
    };
  }, [open, disabled]);
  useEffect(() => {
    if (!open) return;
    const dismiss = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) {
        setOpen(false);
        setQuery("");
      }
    };
    document.addEventListener("pointerdown", dismiss, true);
    return () => document.removeEventListener("pointerdown", dismiss, true);
  }, [open]);
  useEffect(() => {
    popup.current?.querySelector(".picker-options")?.scrollTo({ top: 0 });
  }, [query, open]);
  useLayoutEffect(() => {
    const list = popup.current?.querySelector<HTMLElement>(".picker-options");
    const option = activeId ? document.getElementById(activeId) : null;
    if (!list || !option) return;
    // Reveal an option inside its list without scrolling the page or modal.
    const item = option.getBoundingClientRect(), box = list.getBoundingClientRect();
    if (item.top < box.top) list.scrollTop += item.top - box.top;
    else if (item.bottom > box.bottom) list.scrollTop += item.bottom - box.bottom;
  }, [activeId]);

  function close() {
    setOpen(false);
    setQuery("");
    setActive("");
  }
  function begin() {
    if (disabled) return;
    control.current?.focus({ preventScroll: true });
    setOpen(true);
    setActive("");
  }
  function commit(next: readonly string[], added?: SelectionOption) {
    if (added) setMemory((previous) => mergeOptions(previous, [added]));
    if (multiple)
      props.onChange(
        [...next],
        next.map(
          (value) =>
            known.find((option) => option.value === value) ??
            (added?.value === value ? added : { value, label: value }),
        ),
      );
    else props.onChange(next[0] ?? "", added);
  }
  function choose(option: SelectionOption) {
    if (unavailable(option)) return;
    commit(
      multiple ? toggleSelection(values, option.value) : [option.value],
      option,
    );
    if (!multiple) close();
    control.current?.focus({ preventScroll: true });
  }
  function remove(value: string) {
    commit(values.filter((item) => item !== value));
    control.current?.focus({ preventScroll: true });
  }
  function keydown(event: KeyboardEvent<HTMLInputElement | HTMLButtonElement>) {
    if (event.nativeEvent.isComposing || composing || event.keyCode === 229)
      return;
    if (event.key === "Escape" && open) {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    if (event.key === "Tab") {
      close();
      return;
    }
    if (["ArrowDown", "ArrowUp"].includes(event.key)) {
      event.preventDefault();
      if (!open) {
        begin();
        if (event.key === "ArrowUp") setActive(enabled.at(-1)?.value ?? "");
        return;
      }
      const index = enabled.findIndex(
        (option) => option.value === activeOption?.value,
      );
      setActive(
        enabled[
          Math.max(
            0,
            Math.min(
              enabled.length - 1,
              index + (event.key === "ArrowDown" ? 1 : -1),
            ),
          )
        ]?.value ?? "",
      );
      return;
    }
    if (!searchable && (event.key === "Home" || event.key === "End")) {
      event.preventDefault();
      begin();
      setActive(
        (event.key === "Home" ? enabled[0] : enabled.at(-1))?.value ?? "",
      );
      return;
    }
    if (event.key === "Enter" || (!searchable && event.key === " ")) {
      event.preventDefault();
      if (!open) begin();
      else if (failed) setRetry((n) => n + 1);
      else if (activeOption) choose(activeOption);
      return;
    }
    if (multiple && event.key === "Backspace" && !query && values.length) {
      // Focus the removable chip first: a second key confirms deletion.
      const chip = root.current?.querySelectorAll<HTMLButtonElement>(
        ".selected-tag button:not(:disabled)",
      );
      chip?.[chip.length - 1]?.focus();
      event.preventDefault();
      return;
    }
    if (
      !searchable &&
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      if (!open) begin();
      const now = event.timeStamp;
      typeahead.current = {
        text:
          now - typeahead.current.time > 700
            ? event.key
            : typeahead.current.text + event.key,
        time: now,
      };
      const match = enabled.find((option) =>
        option.label
          .toLocaleLowerCase()
          .startsWith(typeahead.current.text.toLocaleLowerCase()),
      );
      if (match) setActive(match.value);
    }
  }
  const accessible: AriaAttributes = {
    "aria-label": props["aria-label"],
    "aria-labelledby":
      props["aria-labelledby"] ?? (label ? `${id}-label` : undefined),
    "aria-describedby":
      [props["aria-describedby"], describedBy(id, hint, error)]
        .filter(Boolean)
        .join(" ") || undefined,
    "aria-invalid": error ? true : props["aria-invalid"],
    "aria-required": required || undefined,
    "aria-expanded": open && !disabled,
    "aria-controls": listId,
    "aria-haspopup": "listbox",
    "aria-activedescendant": activeId,
  };
  const placeholder =
    props.placeholder ??
    t(
      searchable
        ? "selection.searchPlaceholder"
        : "selection.choosePlaceholder",
    );
  let index = 0;
  return (
    <div
      ref={root}
      className={`picker field ${className}`}
      data-multiple={multiple || undefined}
      data-clearable={clearable || undefined}
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.preventDefault();
          event.stopPropagation();
          close();
          control.current?.focus({ preventScroll: true });
        }
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null))
          close();
      }}
    >
      {label && (
        <label id={`${id}-label`} htmlFor={id} className="field-label">
          {label}
          {required && <span aria-hidden="true"> *</span>}
        </label>
      )}
      <div
        ref={anchor}
        onClick={(event) => {
          if (event.target === event.currentTarget && !disabled) {
            control.current?.focus({ preventScroll: true });
            if (open) close();
            else begin();
          }
        }}
        className={`picker-anchor ${open ? "is-open" : ""} ${disabled ? "is-disabled" : ""} ${error || props["aria-invalid"] ? "is-invalid" : ""}`}
      >
        {multiple &&
          selected.map((option) => (
            <span className="selected-tag" key={option.value}>
              <span className="control-label-text">{option.label}</span>
              <button
                type="button"
                disabled={disabled || option.disabled}
                aria-label={t("selection.remove", { label: option.label })}
                onClick={() => remove(option.value)}
                onKeyDown={(event) => {
                  if (event.key === "Backspace" || event.key === "Delete") {
                    event.preventDefault();
                    remove(option.value);
                  }
                  if (event.key === "ArrowRight") {
                    event.preventDefault();
                    control.current?.focus({ preventScroll: true });
                  }
                }}
              >
                <X size={12} aria-hidden="true" />
              </button>
            </span>
          ))}
        {searchable ? (
          <input
            ref={control}
            id={id}
            className="picker-input"
            title={!open && !multiple ? selected[0]?.label : undefined}
            type="text"
            role="combobox"
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            {...accessible}
            aria-autocomplete="list"
            disabled={disabled}
            placeholder={
              multiple && values.length ? t("selection.addMore") : placeholder
            }
            value={open ? query : multiple ? "" : (selected[0]?.label ?? "")}
            onClick={() => {
              if (!open) begin();
            }}
            onChange={(event) => {
              setQuery(event.target.value);
              setActive("");
              setOpen(true);
            }}
            onCompositionStart={() => setComposing(true)}
            onCompositionEnd={() => setComposing(false)}
            onKeyDown={keydown}
          />
        ) : (
          <button
            ref={control}
            id={id}
            type="button"
            className="picker-trigger"
            title={selected[0]?.label}
            role="combobox"
            {...accessible}
            disabled={disabled}
            onClick={() => (open ? close() : begin())}
            onKeyDown={keydown}
          >
            <span className={!values.length ? "picker-placeholder" : ""}>
              {selected[0]?.label ?? placeholder}
            </span>
          </button>
        )}
        {clearable && !!values.length && (
          <button
            type="button"
            className="picker-clear"
            disabled={disabled || selected.every((option) => option.disabled)}
            aria-label={t("selection.clear", {
              label: label ?? props["aria-label"] ?? t("selection.value"),
            })}
            onClick={() => {
              commit(
                values.filter(
                  (value) =>
                    known.find((option) => option.value === value)?.disabled,
                ),
              );
              setQuery("");
              control.current?.focus({ preventScroll: true });
            }}
          >
            <X size={14} aria-hidden="true" />
          </button>
        )}
        <span className="picker-chevron" aria-hidden="true">
          {waiting && open ? (
            <Spinner size={16} />
          ) : (
            <ChevronDown size={15} />
          )}
        </span>
      </div>
      {required && (
        <input
          className="picker-form-proxy"
          tabIndex={-1}
          aria-hidden="true"
          required
          disabled={disabled}
          value={hasValue ? "selected" : ""}
          onChange={() => {}}
          onInvalid={(event) => {
            event.preventDefault();
            setInvalid(true);
            control.current?.focus({ preventScroll: true });
          }}
        />
      )}
      {name &&
        selected.map((option) => (
          <input
            type="hidden"
            key={option.value}
            name={name}
            value={option.value}
            disabled={disabled}
          />
        ))}
      <div
        ref={popup}
        popover="manual"
        className={`picker-popup${loadOptions ? " picker-popup-async" : ""}`}
        style={{ "--picker-room": "288px" } as CSSProperties}
      >
        {searchable && (
          <div className="picker-status" aria-live="polite" role="status">
            <Search size={14} aria-hidden="true" />
            <span>
              {!enough
                ? t("selection.minQuery", { count: minQueryLength })
                : waiting
                  ? t("selection.loading")
                  : failed
                    ? t("selection.failed")
                    : t("selection.results", { count: visible.length })}
            </span>
          </div>
        )}
        <div
          role="listbox"
          id={listId}
          aria-label={label ?? props["aria-label"] ?? t("selection.value")}
          aria-multiselectable={multiple || undefined}
          aria-busy={waiting || undefined}
          className="picker-options"
        >
          {[...groups.entries()].map(([group, items]) => (
            <div
              role={group ? "group" : undefined}
              aria-label={group || undefined}
              key={group}
            >
              {group && (
                <div className="picker-group" aria-hidden="true">
                  {group}
                </div>
              )}
              {items.map((option) => {
                const optionId = `${id}-option-${index++}`,
                  isSelected = values.includes(option.value);
                return (
                  <div
                    key={option.value}
                    id={optionId}
                    role="option"
                    data-value={option.value}
                    aria-selected={isSelected}
                    aria-disabled={unavailable(option) || undefined}
                    className={`picker-option ${option.value === activeOption?.value ? "is-active" : ""}`}
                    onPointerMove={(event) => {
                      if (event.pointerType === "mouse" && !unavailable(option))
                        setActive(option.value);
                    }}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => choose(option)}
                  >
                    {option.icon && (
                      <span className="selection-icon" aria-hidden="true">
                        {option.icon}
                      </span>
                    )}
                    <span className="picker-option-copy">
                      <span className="control-label-text">{option.label}</span>
                      {option.description && (
                        <span className="selection-description">
                          {option.description}
                        </span>
                      )}
                    </span>
                    <Check
                      size={16}
                      aria-hidden="true"
                      className={isSelected ? "" : "invisible"}
                    />
                  </div>
                );
              })}
            </div>
          ))}
        </div>
        {!visible.length && !waiting && !failed && enough && (
          <div className="picker-empty">{t("selection.noResults")}</div>
        )}
        {failed && (
          <div className="picker-empty">
            <button
              type="button"
              className="text-link"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                setRetry((n) => n + 1);
                control.current?.focus({ preventScroll: true });
              }}
            >
              {t("common.retry")}
            </button>
            <span className="field-note">{t("selection.retryKey")}</span>
          </div>
        )}
        {atLimit && (
          <p className="picker-status" role="status">
            {t("selection.limit", { count: maxSelected ?? 0 })}
          </p>
        )}
        {multiple && (
          <div className="picker-status">
            <span>
              {t("selection.selectedCount", { count: values.length })}
            </span>
            <span>{t("selection.dismissHint")}</span>
          </div>
        )}
      </div>
      <FieldFeedback id={id} hint={hint} error={error} reserveSpace={reserveMessage} />
      <span className="sr-only" aria-live="polite">
        {multiple
          ? t("selection.selectedCount", { count: values.length })
          : (selected[0]?.label ?? "")}
      </span>
    </div>
  );
}
