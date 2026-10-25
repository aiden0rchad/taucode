import { Children, isValidElement, createContext, useContext, useId, useState, useRef, useEffect, useLayoutEffect, type CSSProperties, type ComponentPropsWithoutRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { PopoverSurface } from "../popover/PopoverPrimitives";
import Info from "lucide-react/dist/esm/icons/info";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
const Disabled = createContext(false);
const ControlId = createContext<string | undefined>(undefined);
const ToggleLabel = createContext<string | undefined>(undefined);
import { joinClassNames } from "../classNames";

type SettingsSectionProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function SettingsPage({
  title,
  subtitle,
  className,
  children,
}: SettingsSectionProps) {
  return (
    <section className={joinClassNames("settings-section settings-page", className)}>
      <h2 className="settings-section-title">{title}</h2>
      {subtitle ? <div className="settings-section-subtitle">{subtitle}</div> : null}
      {children}
    </section>
  );
}

export function SettingsSection({ children, ...props }: SettingsSectionProps) {
  return <SettingsPage {...props}><SettingsGroup title="Preferences">{children}</SettingsGroup></SettingsPage>;
}

export function SettingsGroup({ title, subtitle, children, actions, disabled = false, collapsible = false, className, ...props }: Omit<ComponentPropsWithoutRef<"div">, "title"> & { title?: ReactNode; subtitle?: ReactNode; actions?: ReactNode; disabled?: boolean; collapsible?: boolean }) {
  const inherited = useContext(Disabled);
  const [expanded, setExpanded] = useState(false);
  const contentId = useId();
  return <Disabled.Provider value={disabled || inherited}><div {...props} className={joinClassNames("settings-group", className)}>
    {title && <h3 className="settings-group-title">{collapsible ? <button type="button" className="settings-group-disclosure" aria-expanded={expanded} aria-controls={contentId} onClick={() => setExpanded(!expanded)}><ChevronRight size={16} aria-hidden />{title}</button> : title}{actions && <span className="settings-group-actions">{actions}</span>}</h3>}
    {subtitle && <p className="settings-group-description">{subtitle}</p>}
    <div id={contentId} className="settings-group-card" aria-disabled={disabled || inherited || undefined} hidden={collapsible && !expanded}>{children}</div>
  </div></Disabled.Provider>;
}

export function SettingsRow({ children, className, ...props }: ComponentPropsWithoutRef<"div">) {
  return <div {...props} className={joinClassNames("settings-row", className)}>{children}</div>;
}

export function SettingsSelect({ children, className, onChange, value, defaultValue, disabled, multiple, ...props }: ComponentPropsWithoutRef<"select">) {
  const inherited = useContext(Disabled);
  const id = useContext(ControlId);
  const [selected, setSelected] = useState(defaultValue);
  const anchor = useRef<HTMLSelectElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const listId = useId();
  const [active, setActive] = useState<number | null>(null);
  const [position, setPosition] = useState<CSSProperties>({ left: 0, top: 0, width: 320, maxHeight: 300 });
  const options: { value: string; label: ReactNode; description?: string; recommended?: boolean; disabled?: boolean }[] = [];
  const collect = (nodes: ReactNode) => Children.forEach(nodes, node => {
    if (!isValidElement<{ value?: string; children?: ReactNode; label?: string; disabled?: boolean; "data-description"?: string; "data-recommended"?: boolean }>(node)) return;
    if (node.type === "option") options.push({ value: String(node.props.value ?? node.props.children ?? ""), label: node.props.label ?? node.props.children, description: node.props["data-description"], recommended: node.props["data-recommended"], disabled: node.props.disabled });
    else collect(node.props.children);
  });
  collect(children);
  const selectedIndex = options.findIndex(option => option.value === String(value ?? selected ?? options[0]?.value));
  const option = options[selectedIndex];
  const text = (nodes: ReactNode): string => Children.toArray(nodes).map(node => isValidElement<{ children?: ReactNode }>(node) ? text(node.props.children) : String(node)).join("");
  const open = active !== null && !disabled && !inherited;
  useEffect(() => { if (open) document.getElementById(`${listId}-${active}`)?.scrollIntoView?.({ block: "nearest" }); }, [active, listId, open]);
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const rect = anchor.current!.getBoundingClientRect();
      const height = Math.min(320, Math.max(rect.top - 16, window.innerHeight - rect.bottom - 16));
      const width = Math.min(Math.max(rect.width, 360), window.innerWidth - 24);
      setPosition({ left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)), ...(window.innerHeight - rect.bottom >= height ? { top: rect.bottom + 4 } : { bottom: window.innerHeight - rect.top + 4 }), width, maxHeight: height });
    };
    const dismiss = (event: PointerEvent) => { if (!anchor.current?.contains(event.target as Node) && !list.current?.contains(event.target as Node)) setActive(null); };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    document.addEventListener("pointerdown", dismiss);
    return () => { window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); document.removeEventListener("pointerdown", dismiss); };
  }, [open]);
  const choose = (index: number) => {
    if (!anchor.current || options[index]?.disabled) return;
    anchor.current.value = options[index].value;
    anchor.current.dispatchEvent(new Event("change", { bubbles: true }));
    setActive(null);
    anchor.current.focus();
  };
  return <span className={joinClassNames("settings-select-control", multiple && "is-multiple")}>
    {!multiple && <span className="settings-select-value" aria-hidden="true"><span className="settings-select-label">{option?.label}</span><ChevronRight className="settings-select-chevron" size={14} /></span>}
    <select id={id} {...props} ref={anchor} title={[text(option?.label), option?.recommended && "Recommended", option?.description].filter(Boolean).join(" — ")} aria-expanded={multiple ? undefined : open} aria-controls={open ? listId : undefined} aria-activedescendant={open ? `${listId}-${active}` : undefined} className={joinClassNames("settings-select", className)} value={value} defaultValue={defaultValue} multiple={multiple} disabled={disabled || inherited}
      onMouseDown={event => { if (!multiple) { event.preventDefault(); anchor.current?.focus(); setActive(open ? null : Math.max(0, selectedIndex)); } }}
      onBlur={() => setActive(null)}
      onKeyDown={event => {
        if (multiple) return;
        if (event.key === "Escape" && open) { event.preventDefault(); event.stopPropagation(); setActive(null); }
        else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
          event.preventDefault();
          const enabled = options.map((item, index) => item.disabled ? -1 : index).filter(index => index >= 0);
          const current = enabled.indexOf(active ?? selectedIndex);
          setActive(enabled[event.key === "Home" ? 0 : event.key === "End" ? enabled.length - 1 : (current + (event.key === "ArrowUp" ? -1 : 1) + enabled.length) % enabled.length] ?? null);
        } else if (event.key === "Enter" || event.key === " ") { event.preventDefault(); if (open) choose(active!); else setActive(Math.max(0, selectedIndex)); }
      }} onChange={event => { setSelected(event.target.value); setActive(null); onChange?.(event); }}>{children}</select>
    {open && createPortal(<PopoverSurface ref={list} id={listId} role="listbox" aria-label={props["aria-label"] ?? anchor.current?.labels?.[0]?.textContent ?? "Options"} className="settings-select-options" style={{ position: "fixed", ...position, zIndex: anchor.current?.closest(".ds-modal") ? "var(--z-tooltip)" : "var(--z-popover)" }} onMouseDown={event => event.preventDefault()}>{options.map((item, index) => <div key={item.value} id={`${listId}-${index}`} role="option" aria-selected={index === selectedIndex} aria-disabled={item.disabled || undefined} data-active={index === active} onMouseEnter={() => setActive(index)} onClick={() => choose(index)}><span>{item.label}{item.recommended && <small className="settings-option-pill">Recommended</small>}</span>{item.description && <small className="settings-option-description">{item.description}</small>}</div>)}</PopoverSurface>, document.body)}
  </span>;
}

export function SettingsTextField({ className, disabled, ...props }: ComponentPropsWithoutRef<"input">) {
  const inherited = useContext(Disabled);
  const id = useContext(ControlId);
  return <input id={id} {...props} disabled={disabled || inherited} className={joinClassNames("settings-input", className)} />;
}
export function SettingsTextArea({ className, disabled, ...props }: ComponentPropsWithoutRef<"textarea">) {
  const inherited = useContext(Disabled);
  const id = useContext(ControlId);
  return <textarea id={id} {...props} disabled={disabled || inherited} className={joinClassNames("settings-textarea", className)} />;
}
export function SettingsButton({ className, disabled, type = "button", ...props }: ComponentPropsWithoutRef<"button">) {
  const inherited = useContext(Disabled);
  return <button {...props} type={type} disabled={disabled || inherited} className={joinClassNames("settings-button", className)} />;
}
export function SettingsNumberStepper(props: ComponentPropsWithoutRef<"input">) {
  return <SettingsTextField {...props} type="number" />;
}
export function SettingsSegmented({ label, value, options, onChange, tabs = false }: { label: string; value: string; options: { value: string; label: string; disabled?: boolean }[]; onChange: (value: string) => void; tabs?: boolean }) {
  return <div className="settings-segmented" role={tabs ? "tablist" : "group"} aria-label={label} onKeyDown={event => {
    if (!tabs || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const enabled = options.filter(option => !option.disabled);
    const index = enabled.findIndex(option => option.value === value);
    const next = event.key === "Home" ? 0 : event.key === "End" ? enabled.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + enabled.length) % enabled.length;
    onChange(enabled[next].value);
    event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")[next]?.focus();
  }}>{options.map(option => <SettingsButton key={option.value} role={tabs ? "tab" : undefined} tabIndex={tabs && value !== option.value ? -1 : 0} className="settings-segmented-option" aria-selected={tabs ? value === option.value : undefined} aria-pressed={tabs ? undefined : value === option.value} disabled={option.disabled} onClick={() => onChange(option.value)}>{option.label}</SettingsButton>)}</div>;
}

type SettingsSubsectionProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
};

export function SettingsSubsection({ title, subtitle, className }: SettingsSubsectionProps) {
  return (
    <div className={className}>
      <div className="settings-subsection-title">{title}</div>
      {subtitle ? <div className="settings-subsection-subtitle">{subtitle}</div> : null}
    </div>
  );
}

type SettingsToggleRowProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function SettingsToggleRow({
  title,
  subtitle,
  className,
  children,
}: SettingsToggleRowProps) {
  const labelId = useId();
  return (
    <ToggleLabel.Provider value={labelId}>
    <SettingsRow className={joinClassNames("settings-toggle-row", className)}>
      <div>
        <div id={labelId} className="settings-toggle-title">{title}</div>
        {subtitle ? <div className="settings-toggle-subtitle">{subtitle}</div> : null}
      </div>
      {children}
    </SettingsRow></ToggleLabel.Provider>
  );
}

type SettingsToggleSwitchProps = Omit<
  ComponentPropsWithoutRef<"button">,
  "type" | "children" | "className" | "aria-pressed"
> & {
  pressed: boolean;
  className?: string;
};

export function SettingsToggleSwitch({
  pressed,
  className,
  ...props
}: SettingsToggleSwitchProps) {
  const labelId = useContext(ToggleLabel);
  const disabled = useContext(Disabled);
  return (
    <button
      type="button"
      className={joinClassNames("settings-toggle", pressed && "on", className)}
      aria-pressed={pressed}
      aria-labelledby={props["aria-label"] ? undefined : labelId}
      {...props}
      disabled={disabled || props.disabled}
    >
      <span className="settings-toggle-knob" />
    </button>
  );
}

export function SettingsFieldRow({ title, subtitle, help, children, className, ...props }: Omit<ComponentPropsWithoutRef<"label">, "title"> & { title: ReactNode; subtitle?: ReactNode; help?: ReactNode }) {
  const generatedId = useId();
  const id = props.htmlFor ?? generatedId;
  return <SettingsRow className="settings-control-row"><div className={joinClassNames("settings-field-row", className)}><div className="settings-field-info"><label {...props} htmlFor={id} className="settings-toggle-title">{title}</label>{subtitle && <span className="settings-toggle-subtitle">{subtitle}</span>}</div><ControlId.Provider value={id}>{children}</ControlId.Provider></div>{help && <div className="settings-control-help settings-callout"><Info size={14} aria-hidden />{help}</div>}</SettingsRow>;
}
