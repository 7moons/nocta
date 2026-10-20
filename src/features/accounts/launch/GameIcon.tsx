import { Gamepad2 } from "lucide-react";
import { useT } from "@/i18n";
import { usePlaceInfo } from "@/lib/placeInfo";

/** A game icon, or a neutral placeholder while there is none. */
export function GameThumb({
  src,
  alt = "",
  size = 16,
}: {
  src: string | null | undefined;
  alt?: string;
  size?: number;
}) {
  return src ? (
    <img src={src} alt={alt} />
  ) : (
    <span className="game-icon-fallback">
      <Gamepad2 size={size} />
    </span>
  );
}

// Icons and names come from the shared place cache, so lists cost no repeat lookups.
export function GameIcon({ placeId }: { placeId: string }) {
  const info = usePlaceInfo(placeId);
  return <GameThumb src={info?.iconUrl} />;
}

export function PlaceName({ placeId }: { placeId: string }) {
  const t = useT();
  const info = usePlaceInfo(placeId);
  return <>{info?.name ?? t("launch.game.placeFallback", { id: placeId })}</>;
}
