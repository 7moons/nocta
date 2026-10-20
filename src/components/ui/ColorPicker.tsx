import { useState, type KeyboardEvent, type PointerEvent } from "react";
import { hexToHsv, hsvToHex, type Hsv } from "@/lib/color";

const clamp = (value: number) => Math.min(1, Math.max(0, value));

/**
 * Any color: saturation and brightness in the square, hue on the strip. The hue is kept while
 * the color is gray, so moving back out of gray returns to it.
 */
export function ColorPicker({
  value,
  onChange,
  labels,
}: {
  value: string;
  onChange: (hex: string) => void;
  labels: { area: string; hue: string };
}) {
  const [hsv, setHsv] = useState<Hsv>(() => hexToHsv(value));
  // A hex typed elsewhere moves the picker, unless it is the color the picker made itself.
  if (hsvToHex(hsv) !== value.toLowerCase()) {
    const next = hexToHsv(value);
    setHsv(next.s === 0 ? { ...next, h: hsv.h } : next);
  }
  const change = (next: Hsv) => {
    setHsv(next);
    onChange(hsvToHex(next));
  };
  const pick = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    change({
      h: hsv.h,
      s: clamp((event.clientX - box.left) / box.width),
      v: clamp(1 - (event.clientY - box.top) / box.height),
    });
  };
  const step = (event: KeyboardEvent<HTMLDivElement>) => {
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-0.02, 0],
      ArrowRight: [0.02, 0],
      ArrowUp: [0, 0.02],
      ArrowDown: [0, -0.02],
    };
    const move = moves[event.key];
    if (!move) return;
    event.preventDefault();
    change({ h: hsv.h, s: clamp(hsv.s + move[0]), v: clamp(hsv.v + move[1]) });
  };

  return (
    <div className="color-picker">
      <div
        className="color-area"
        role="slider"
        tabIndex={0}
        aria-label={labels.area}
        aria-valuetext={value}
        style={{ backgroundColor: `hsl(${hsv.h} 100% 50%)` }}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          pick(event);
        }}
        onPointerMove={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId))
            pick(event);
        }}
        onKeyDown={step}
      >
        <span
          className="color-area-thumb"
          style={{
            left: `${hsv.s * 100}%`,
            top: `${(1 - hsv.v) * 100}%`,
            background: value,
          }}
        />
      </div>
      <input
        className="color-hue"
        type="range"
        min={0}
        max={359}
        aria-label={labels.hue}
        value={Math.round(hsv.h)}
        onChange={(event) => change({ ...hsv, h: Number(event.target.value) })}
      />
    </div>
  );
}
