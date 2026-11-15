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
  return (
    <Tag
      className={className ? styles.root + " " + className : styles.root}
      title={short ? value : undefined}
      aria-label={short ? value : undefined}
    >
      {short ? value.slice(0, 6) + "…" + value.slice(-4) : value}
    </Tag>
  );
}
