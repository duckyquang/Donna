import type { ButtonHTMLAttributes, HTMLAttributes, InputHTMLAttributes, ReactNode } from "react";

export function Button({
  children,
  variant = "primary",
  size = "md",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "danger" | "success";
  size?: "sm" | "md";
  children: ReactNode;
}) {
  const base =
    "inline-flex items-center justify-center gap-2 rounded font-medium transition-all disabled:cursor-not-allowed disabled:opacity-40 select-none";
  const sizes = {
    sm: "px-3 py-1.5 text-xs",
    md: "px-4 py-2 text-sm",
  };
  const variants = {
    primary: "bg-donna-accent text-white hover:bg-donna-accent-hover shadow-sm",
    ghost: "border border-donna-border-strong text-donna-text-secondary hover:bg-donna-surface-hover hover:text-donna-text",
    danger: "border border-donna-danger/30 text-red-400 hover:bg-donna-danger-dim",
    success: "border border-donna-success/30 text-green-400 hover:bg-donna-success-dim",
  };
  return (
    <button className={`${base} ${sizes[size]} ${variants[variant]} ${className}`} {...props}>
      {children}
    </button>
  );
}

export function Input({
  className = "",
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={`w-full rounded border border-donna-border bg-donna-surface px-3 py-2 text-sm text-donna-text placeholder-donna-muted outline-none transition-colors focus:border-donna-border-strong focus:bg-donna-surface-raised ${className}`}
      {...props}
    />
  );
}

export function Badge({
  children,
  variant = "default",
  className = "",
}: {
  children: ReactNode;
  variant?: "default" | "success" | "warning" | "danger" | "accent";
  className?: string;
}) {
  const variants = {
    default: "bg-donna-surface-raised text-donna-text-secondary border-donna-border",
    success: "bg-donna-success-dim text-green-400 border-green-500/20",
    warning: "bg-amber-500/10 text-amber-400 border-amber-500/20",
    danger: "bg-donna-danger-dim text-red-400 border-red-500/20",
    accent: "bg-donna-accent-dim text-donna-accent-light border-donna-accent/20",
  };
  return (
    <span className={`inline-flex items-center rounded border px-1.5 py-0.5 text-xs font-medium ${variants[variant]} ${className}`}>
      {children}
    </span>
  );
}

export function Card({
  children,
  className = "",
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`rounded-lg border border-donna-border bg-donna-surface shadow-card ${className}`} {...props}>
      {children}
    </div>
  );
}

export function Spinner({ className = "" }: { className?: string }) {
  return (
    <span
      className={`inline-block h-4 w-4 animate-spin rounded-full border-2 border-donna-accent/20 border-t-donna-accent ${className}`}
    />
  );
}

export function ThinkingDots({ className = "" }: { className?: string }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 ${className}`}
      role="status"
      aria-label="Donna is thinking"
    >
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="thinking-dot h-1.5 w-1.5 rounded-full bg-donna-accent-light"
          style={{ animationDelay: `${i * 0.15}s` }}
        />
      ))}
    </span>
  );
}

export function Divider({ className = "" }: { className?: string }) {
  return <div className={`border-t border-donna-border ${className}`} />;
}

export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
      {icon && <div className="text-donna-muted">{icon}</div>}
      <div>
        <p className="text-sm font-medium text-donna-text">{title}</p>
        {description && <p className="mt-1 text-xs text-donna-muted">{description}</p>}
      </div>
      {action}
    </div>
  );
}
