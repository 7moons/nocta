import { Check } from "lucide-react";
import { readableOn } from "@/lib/color";

/** A round color sample; the chosen one carries a check mark in a color that reads on it. */
export function ColorDot({
  color,
  chosen = false,
  size = 16,
}: {
  color: string;
  chosen?: boolean;
  size?: number;
}) {
  return (
    <span
      className="color-dot"
      style={{ background: color, width: size, height: size }}
    >
      {chosen && (
        <Check size={size - 6} strokeWidth={3} color={readableOn(color)} />
      )}
    </span>
  );
}
