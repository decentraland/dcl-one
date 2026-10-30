import { useEffect, useState } from "react";
import { bootClockElapsed, bootClockRelease, bootClockStart, formatElapsed } from "../boot-machine";
import "./dcleditorchrome.css";
import "./dcleditorchrome-shell.css";

export default function EditorBootShell({ text = "Loading scene editor\u{2026}" }: { text?: string }) {
  const [elapsedS, setElapsedS] = useState(0);
  useEffect(() => {
    bootClockStart();
    setElapsedS(bootClockElapsed());
    const id = setInterval(() => setElapsedS(bootClockElapsed()), 1000);
    return () => {
      clearInterval(id);
      bootClockRelease();
    };
  }, []);
  return (
    <div className="eui-root eui-viewport eui-boot-shell ui2" role="region" aria-label="DCL Editor">
      <div className="eui-boot" role="status">
        <span className="eui-boot-spinner" aria-hidden="true" />
        <span className="eui-boot-stage">{text}</span>
        {elapsedS >= 1 ? <span className="eui-boot-elapsed">{formatElapsed(elapsedS)}</span> : null}
      </div>
    </div>
  );
}
