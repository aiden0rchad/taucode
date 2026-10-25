import { Component, Fragment, type ReactNode, type ErrorInfo } from "react";
import { record, diagnosticsEnabled } from "./client";
export class DiagnosticBoundary extends Component<{ panel: string; children: ReactNode }, { failed: boolean; generation: number }> {
  state = { failed: false, generation: 0 };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(_error: Error, info: ErrorInfo) {
    if (diagnosticsEnabled()) record({ kind: "error", area: "ui", code: "ui.render_crash", message: "Panel render failed", context: { panel: this.props.panel }, detail: { stack: info.componentStack } });
  }
  render() {
    return this.state.failed ? <div role="alert">This panel hit an error · <button type="button" onClick={() => this.setState(s => ({ failed: false, generation: s.generation + 1 }))}>Reload panel</button></div> : <Fragment key={this.state.generation}>{this.props.children}</Fragment>;
  }
}
