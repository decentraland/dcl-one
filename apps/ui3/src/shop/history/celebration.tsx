// from: shop@2d3bf62:app/src/components/Confetti, app/src/components/Icons/JumpInIcon.tsx
// The purchase celebration both success surfaces share: the Success page and the Buy now dialog.
import { useEffect, useRef } from "react";

import { useMediaQuery } from "../../data/hooks/useMediaQuery";

export const JumpInIcon = () => (
  <svg width="28" height="28" viewBox="0 0 28 28" fill="none" aria-hidden="true">
    <rect x="0.5" y="0.5" width="27" height="27" rx="7.5" fill="rgba(255,255,255,0.16)" stroke="rgba(255,255,255,0.3)" />
    <path d="M11 9h8v8M19 9l-10 10" stroke="#fcfcfc" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const CONFETTI_COLORS = ["#ff2d55", "#ff7439", "#c640cd", "#691fa9", "#34ce77", "#ffc9d5", "#fcfcfc", "#ffbc5b"];

export function Confetti({ className = "shop-success__confetti" }: { className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const play = !useMediaQuery("(prefers-reduced-motion: reduce)", true);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext?.("2d");
    if (!play || !canvas || !ctx) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = window.innerWidth;
    const h = window.innerHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    ctx.scale(dpr, dpr);
    type P = { x: number; y: number; vx: number; vy: number; rot: number; vr: number; size: number; color: string; shape: number; born: number };
    const parts: P[] = [];
    const fire = (at: number) => {
      for (const side of [-1, 1]) {
        for (let i = 0; i < 70; i++) {
          const tilt = 0.2 + Math.random() * 0.6;
          const speed = (0.9 + Math.random() * 0.8) * Math.max(h, 600) * 0.0165;
          parts.push({
            x: side < 0 ? 0 : w,
            y: h * 0.85,
            vx: -side * Math.sin(tilt) * speed,
            vy: -Math.cos(tilt) * speed,
            rot: Math.random() * Math.PI,
            vr: (Math.random() - 0.5) * 0.3,
            size: 6 + Math.random() * 7,
            color: CONFETTI_COLORS[Math.floor(Math.random() * CONFETTI_COLORS.length)]!,
            shape: Math.random(),
            born: at,
          });
        }
      }
    };
    let raf = 0;
    const start = performance.now();
    let second = false;
    fire(0);
    const frame = (now: number) => {
      const t = now - start;
      if (!second && t > 900) {
        second = true;
        fire(t);
      }
      ctx.clearRect(0, 0, w, h);
      let alive = 0;
      for (const p of parts) {
        const age = t - p.born;
        if (age < 0 || age > 3600) continue;
        p.vy += 0.32;
        p.vx *= 0.985;
        p.vy *= 0.985;
        p.x += p.vx;
        p.y += p.vy;
        p.rot += p.vr;
        if (p.y > h + 40) continue;
        alive++;
        ctx.save();
        ctx.globalAlpha = age > 2800 ? Math.max(0, 1 - (age - 2800) / 800) : 1;
        ctx.translate(p.x, p.y);
        ctx.rotate(p.rot);
        ctx.fillStyle = p.color;
        if (p.shape < 0.5) ctx.fillRect(-p.size / 2, -p.size / 4, p.size, p.size / 2);
        else {
          ctx.beginPath();
          ctx.arc(0, 0, p.size / 3, 0, Math.PI * 2);
          ctx.fill();
        }
        ctx.restore();
      }
      if (alive > 0 || t < 1000) raf = requestAnimationFrame(frame);
      else ctx.clearRect(0, 0, w, h);
    };
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  }, [play]);
  if (!play) return null;
  return <canvas ref={ref} className={className} aria-hidden="true" data-testid="confetti" />;
}
