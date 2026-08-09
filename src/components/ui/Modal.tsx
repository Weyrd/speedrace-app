import type { ReactNode } from "react";
import { Dialog } from "radix-ui";
import { cn } from "../../lib/utils";

interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  className?: string;
  children: ReactNode;
}

export function Modal({ open, onOpenChange, className, children }: ModalProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-modal bg-black/70" />
        <Dialog.Content className="fixed inset-0 z-modal flex items-center justify-center outline-none">
          <div
            className={cn(
              "bg-bg1 border border-border rounded-sm mx-3 p-3.5 w-full max-w-xs",
              className,
            )}
          >
            {children}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

Modal.Title = Dialog.Title;
Modal.Description = Dialog.Description;
