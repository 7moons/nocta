import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";

export type Removal = "trash" | "purge";

/** Confirms moving accounts to Trash, or deleting them for good from Trash. */
export function RemoveAccountsDialog({
  removal,
  count,
  onConfirm,
  onClose,
}: {
  removal: Removal | null;
  count: number;
  onConfirm: (removal: Removal) => void;
  onClose: () => void;
}) {
  const t = useT();
  const purge = removal === "purge";
  return (
    <Dialog
      open={removal !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t(
        purge ? "accounts.remove.purgeTitle" : "accounts.remove.trashTitle",
      )}
      description={t(
        purge ? "accounts.remove.purgeDetail" : "accounts.remove.trashDetail",
      )}
    >
      <div className="dialog-body">
        <p>{t("accounts.remove.count", { count })}</p>
        <div className="dialog-footer">
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              if (removal) onConfirm(removal);
              onClose();
            }}
          >
            {t(purge ? "accounts.remove.purge" : "accounts.remove.trash")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
