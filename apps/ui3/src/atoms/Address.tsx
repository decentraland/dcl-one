import styles from "./Address.module.css";

export type AddressProps = {
  value: string;
  shorten?: boolean;
  strong?: boolean;
  className?: string;
};

export default function Address({ value, shorten = true, strong = false, className }: AddressProps) {
  const Tag = strong ? "strong" : "span";
  const short = shorten && value.length > 11;
  const root = className ? styles.root + " " + className : styles.root;
  if (!short) return <Tag className={root}>{value}</Tag>;
  // A plain element has no name of its own: a screen reader reads the full address as text, the eye sees the short one.
  return (
    <Tag className={root} title={value}>
      <span aria-hidden="true">{value.slice(0, 6) + "…" + value.slice(-4)}</span>
      <span className={styles.full}>{value}</span>
    </Tag>
  );
}
