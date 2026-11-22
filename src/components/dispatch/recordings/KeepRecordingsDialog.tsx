// Dispatch: the "are you sure" before old recordings move to the Recycle Bin.
import React from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import type { AudioCleanup } from "@/bindings";
import { Dialog } from "../../ui/Dialog";
import { Button } from "../../ui/Button";
import { lengthLabel, sizeLabel } from "./useKeepRecordings";

interface KeepRecordingsDialogProps {
  pending: AudioCleanup | null;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export const KeepRecordingsDialog: React.FC<KeepRecordingsDialogProps> = ({
  pending,
  busy,
  onConfirm,
  onCancel,
}) => {
  const { t } = useTranslation();
  return (
    <Dialog
      open={pending !== null}
      onOpenChange={(next) => !next && onCancel()}
      title={t("dispatch.recordings.confirm.title", {
        count: pending?.recordings ?? 0,
      })}
      closeLabel={t("dispatch.recordings.confirm.cancel")}
      footer={
        <div className="flex w-full flex-wrap justify-end gap-3">
          <Button variant="secondary" size="md" onClick={onCancel}>
            {t("dispatch.recordings.confirm.cancel")}
          </Button>
          <Button
            variant="danger"
            size="md"
            onClick={onConfirm}
            disabled={busy}
          >
            <span className="flex items-center gap-2">
              <Trash2 className="h-4 w-4" aria-hidden="true" />
              {t("dispatch.recordings.confirm.confirm")}
            </span>
          </Button>
        </div>
      }
    >
      {pending && (
        <div className="flex flex-col gap-3 text-sm leading-relaxed text-ink-2">
          <p className="m-0">
            {t("dispatch.recordings.confirm.body", {
              kept: pending.kept,
              count: pending.recordings,
              length: lengthLabel(pending.duration_ms, t),
              size: sizeLabel(pending.bytes),
            })}
          </p>
          <p className="m-0 rounded-lg border border-line bg-background px-3.5 py-2.5 text-[13px]">
            {t("dispatch.recordings.confirm.note")}
          </p>
        </div>
      )}
    </Dialog>
  );
};
