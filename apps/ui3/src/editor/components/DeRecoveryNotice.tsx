export default function DeRecoveryNotice({ error, notice, dismiss }: {
  error: string | null;
  notice: string | null;
  dismiss: () => void;
}) {
  if (!error && !notice) return null;
  return <div role={error ? "alert" : "status"} className="eui-recovery-notice">
    <span>{error ?? notice}</span>
    <button className="eui-btn" type="button" onClick={dismiss}>Dismiss</button>
  </div>;
}
