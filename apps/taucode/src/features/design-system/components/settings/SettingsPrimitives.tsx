import { createContext, useContext, useId, type ComponentPropsWithoutRef, type ReactNode } from "react";
const ToggleLabel = createContext<string | undefined>(undefined);
import { joinClassNames } from "../classNames";

type SettingsSectionProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function SettingsSection({
  title,
  subtitle,
  className,
  children,
}: SettingsSectionProps) {
  return (
    <section className={joinClassNames("settings-section", className)}>
      <div className="settings-section-title">{title}</div>
      {subtitle ? <div className="settings-section-subtitle">{subtitle}</div> : null}
      {children}
    </section>
  );
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
    <div className={joinClassNames("settings-toggle-row", className)}>
      <div>
        <div id={labelId} className="settings-toggle-title">{title}</div>
        {subtitle ? <div className="settings-toggle-subtitle">{subtitle}</div> : null}
      </div>
      {children}
    </div></ToggleLabel.Provider>
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
  return (
    <button
      type="button"
      className={joinClassNames("settings-toggle", pressed && "on", className)}
      aria-pressed={pressed}
      aria-labelledby={props["aria-label"] ? undefined : labelId}
      {...props}
    >
      <span className="settings-toggle-knob" />
    </button>
  );
}

export function SettingsFieldRow({ title, subtitle, children, className, ...props }: Omit<ComponentPropsWithoutRef<"label">, "title"> & { title: ReactNode; subtitle?: ReactNode }) {
  return <label {...props} className={joinClassNames("settings-field-row", className)}><span className="settings-field-info"><span className="settings-toggle-title">{title}</span>{subtitle && <span className="settings-toggle-subtitle">{subtitle}</span>}</span>{children}</label>;
}
