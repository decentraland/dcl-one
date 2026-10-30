import Button from "../../atoms/Button";
import "./jumpinview.css";

type JumpInViewProps = {
  placeTitle: string;
  launching: boolean;
  launched: boolean;
  errored: boolean;
  error?: string;
  onStart: () => void;
  onRetry: () => void;
};

export default function JumpInView({
  placeTitle,
  launching,
  launched,
  errored,
  error,
  onStart,
  onRetry,
}: JumpInViewProps) {
  return (
    <div className="jumpin">
      <Button
        variant="primary"
        className="jumpin__cta"
        disabled={launching}
        aria-busy={launching || undefined}
        onClick={onStart}
      >
        {launching ? "LAUNCHING\u{2026}" : "JUMP IN"}
      </Button>

      {launched && (
        <p className="jumpin__status" role="status">
          Launching {placeTitle}&#x2026;
        </p>
      )}

      {errored && (
        <div className="jumpin__error" role="alert">
          <p>Couldn&#x2019;t start the jump: {error ?? "unknown error"}</p>
          <Button
            variant="secondary"
            className="jumpin__retry"
            onClick={onRetry}
          >
            RETRY
          </Button>
        </div>
      )}
    </div>
  );
}
