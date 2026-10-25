import { Squircle } from "@/features/design-system/components/squircle/Squircle";

type WorkspaceGroupProps = {
  toggleId: string | null;
  name: string;
  showHeader: boolean;
  isCollapsed: boolean;
  ungrouped?: boolean;
  isSelected?: boolean;
  onToggleCollapse: (groupId: string) => void;
  children: React.ReactNode;
};

export function WorkspaceGroup({
  toggleId,
  name,
  showHeader,
  isCollapsed,
  ungrouped = false,
  isSelected = false,
  onToggleCollapse,
  children,
}: WorkspaceGroupProps) {
  const isToggleable = Boolean(toggleId);
  const Content = ungrouped ? Squircle : "div";
  return (
    <div className="workspace-group">
      {showHeader && (
        <div
          className={`workspace-group-header${isToggleable ? " is-toggleable" : ""}`}
          onClick={
            toggleId
              ? () => {
                  onToggleCollapse(toggleId);
                }
              : undefined
          }
          onKeyDown={
            toggleId
              ? (event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onToggleCollapse(toggleId);
                  }
                }
              : undefined
          }
          role={isToggleable ? "button" : undefined}
          aria-label={isToggleable ? `${isCollapsed ? "Expand" : "Collapse"} group` : undefined}
          aria-expanded={isToggleable ? !isCollapsed : undefined}
          tabIndex={isToggleable ? 0 : undefined}
        >
          <div className="workspace-group-label">{name}</div>
          {isToggleable && (
            <button
              className={`group-toggle ${isCollapsed ? "" : "expanded"}`}
              onClick={(event) => {
                event.stopPropagation();
                if (!toggleId) {
                  return;
                }
                onToggleCollapse(toggleId);
              }}
              aria-label={isCollapsed ? "Expand group" : "Collapse group"}
              aria-expanded={!isCollapsed}
              type="button"
            >
              <span className="group-toggle-icon">›</span>
            </button>
          )}
        </div>
      )}
      <div className={`workspace-group-list ${isCollapsed ? "collapsed" : ""}`}>
        <Content className={`workspace-group-content${ungrouped ? ` project-group-card ungrouped-card${isSelected ? " is-selected" : ""}` : ""}`}>{children}</Content>
      </div>
    </div>
  );
}
