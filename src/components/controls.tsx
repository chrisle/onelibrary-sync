/**
 * The controls RBXport's Backups pane is built from, drawn the same way:
 * a section with a title, a button and a checkbox.
 */
import type { ReactNode } from "react";

import styles from "./controls.module.css";

export function Section({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <section className={styles.section} aria-label={title}>
      <h3 className={styles.title}>{title}</h3>
      {children}
    </section>
  );
}

export function Button({ children, onClick, disabled, className, label }: {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  className?: string | undefined;
  /** The accessible name when the text is not the whole story. */
  label?: string;
}) {
  return (
    <button
      type="button"
      className={className ? `${styles.button} ${className}` : styles.button}
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
    >
      {children}
    </button>
  );
}

export function Checkbox({ checked, onChange, disabled, label }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <input
      type="checkbox"
      className={styles.checkbox}
      checked={checked}
      disabled={disabled}
      aria-label={label}
      onChange={(event) => onChange(event.target.checked)}
    />
  );
}
