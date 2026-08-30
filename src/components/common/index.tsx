/**
 * Shared UX components for consistent empty/loading/error states.
 */

interface EmptyStateProps {
  title: string;
  description?: string;
  action?: string;
  onAction?: () => void;
}

export function EmptyState({ title, description, action, onAction }: EmptyStateProps) {
  return (
    <div className="ux-empty-state">
      <h3 className="ux-empty-state__title">{title}</h3>
      {description && <p className="ux-empty-state__desc">{description}</p>}
      {action && onAction && (
        <button className="ghost-btn ux-empty-state__action" onClick={onAction}>{action}</button>
      )}
    </div>
  );
}

interface LoadingStateProps {
  message?: string;
}

export function LoadingState({ message = "Loading…" }: LoadingStateProps) {
  return (
    <div className="ux-loading-state">
      <div className="loading-bar" />
      <p className="ux-loading-state__msg">{message}</p>
    </div>
  );
}

interface ErrorStateProps {
  title?: string;
  message: string;
  action?: string;
  onAction?: () => void;
}

export function ErrorState({ title = "Error", message, action, onAction }: ErrorStateProps) {
  return (
    <div className="ux-error-state">
      <h4 className="ux-error-state__title">{title}</h4>
      <p className="ux-error-state__msg">{message}</p>
      {action && onAction && (
        <button className="ghost-btn" onClick={onAction}>{action}</button>
      )}
    </div>
  );
}

interface StatusBadgeProps {
  status: "ready" | "warning" | "error" | "info" | "disabled";
  label: string;
}

export function StatusBadge({ status, label }: StatusBadgeProps) {
  return <span className={`ux-status-badge ux-status-badge--${status}`}>{label}</span>;
}

interface SectionHeaderProps {
  title: string;
  badge?: string;
  badgeStatus?: StatusBadgeProps["status"];
}

export function SectionHeader({ title, badge, badgeStatus }: SectionHeaderProps) {
  return (
    <div className="ux-section-header">
      <h4>{title}</h4>
      {badge && <StatusBadge status={badgeStatus ?? "info"} label={badge} />}
    </div>
  );
}
