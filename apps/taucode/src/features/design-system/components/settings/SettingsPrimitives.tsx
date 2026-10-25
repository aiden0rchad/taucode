import { Children, isValidElement, createContext, useContext, useId, useState, type ComponentPropsWithoutRef, type ReactNode } from "react";
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

export function SettingsGroup({ title, subtitle, children, disabled = false, collapsible = false, className, ...props }: Omit<ComponentPropsWithoutRef<"div">, "title"> & { title?: ReactNode; subtitle?: ReactNode; disabled?: boolean; collapsible?: boolean }) {
  const inherited = useContext(Disabled);
  const [expanded, setExpanded] = useState(false);
  const contentId = useId();
  return <Disabled.Provider value={disabled || inherited}><div {...props} className={joinClassNames("settings-group", className)}>
    {title && <h3 className="settings-group-title">{collapsible ? <button type="button" className="settings-group-disclosure" aria-expanded={expanded} aria-controls={contentId} onClick={() => setExpanded(!expanded)}><ChevronRight size={16} aria-hidden />{title}</button> : title}</h3>}
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
  const options: { value: string; label: ReactNode }[] = [];
  const collect = (nodes: ReactNode) => Children.forEach(nodes, node => {
    if (!isValidElement<{ value?: string; children?: ReactNode }>(node)) return;
    if (node.type === "option") options.push({ value: String(node.props.value ?? node.props.children ?? ""), label: node.props.children });
    else collect(node.props.children);
  });
  collect(children);
  const label = options.find(option => option.value === String(value ?? selected ?? options[0]?.value))?.label;
  return <span className={joinClassNames("settings-select-control", multiple && "is-multiple")}>
    {!multiple && <span className="settings-select-value" aria-hidden="true">{label}<span className="settings-select-chevron">⌄</span></span>}
    <select id={id} {...props} className={joinClassNames("settings-select", className)} value={value} defaultValue={defaultValue} multiple={multiple} disabled={disabled || inherited} onChange={event => { setSelected(event.target.value); onChange?.(event); }}>{children}</select>
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
  return <SettingsRow className="settings-control-row"><div className={joinClassNames("settings-field-row", className)}><div className="settings-field-info"><label {...props} htmlFor={id} className="settings-toggle-title">{title}</label>{subtitle && <span className="settings-toggle-subtitle">{subtitle}</span>}</div><ControlId.Provider value={id}>{children}</ControlId.Provider></div>{help && <div className="settings-control-help">{help}</div>}</SettingsRow>;
}
