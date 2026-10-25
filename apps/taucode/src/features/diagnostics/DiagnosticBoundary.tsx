import "./diagnostics.css";
import { PanelLeftClose } from "lucide-react";
import { SettingsButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { Component, Fragment, type ReactNode, type ErrorInfo } from "react";
import { record, diagnosticsEnabled } from "./client";
function PreviewCrash({ panel }: { panel: string }) {
  if (import.meta.env.MODE === "uipreview" && new URLSearchParams(window.location.search).get("crash") === panel) throw new Error("Preview panel crash");
  return null;
}
export class DiagnosticBoundary extends Component<{ panel: string; children: ReactNode }, { failed: boolean; generation: number }> {
  state = { failed: false, generation: 0 };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(_error: Error, info: ErrorInfo) {
    if (diagnosticsEnabled()) record({ kind: "error", area: "ui", code: "ui.render_crash", message: "Panel render failed", context: { panel: this.props.panel }, detail: { stack: info.componentStack } });
  }
  render() {
    return this.state.failed ? <div role="alert" className="diagnostics-fallback"><PanelLeftClose size={28} aria-hidden /><p>This panel hit an error.<br />Reload it to continue.</p><SettingsButton onClick={() => this.setState(s => ({ failed: false, generation: s.generation + 1 }))}>Reload panel</SettingsButton></div> : <Fragment key={this.state.generation}>{this.state.generation === 0 && <PreviewCrash panel={this.props.panel} />}{this.props.children}</Fragment>;
  }
}
