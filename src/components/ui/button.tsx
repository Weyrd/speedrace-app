import * as React from "react";
import { cva } from "class-variance-authority";
import { Slot } from "radix-ui";
import type { VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 font-mono tracking-wide rounded-sm transition-colors cursor-pointer whitespace-nowrap outline-none focus-visible:ring-1 focus-visible:ring-accent/50 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        ghost: "text-dim hover:text-muted bg-transparent",
        outline:
          "border border-border text-muted bg-transparent hover:border-muted hover:text-text",
        default: "bg-bg2 text-text font-semibold hover:bg-bg0",
        primary: "bg-accent text-on-accent hover:bg-accent-hover",
        success: "bg-green text-white hover:opacity-90",
        destructive: "bg-red text-white hover:opacity-90",
        danger:
          "border border-border text-red bg-transparent hover:border-red/60",
        finish: "bg-green text-white font-semibold hover:bg-green/90",
        forfeit: "bg-red/10 text-red font-semibold hover:bg-red/20",
        start:
          "text-accent border border-accent-dim bg-transparent hover:border-accent hover:bg-accent-dim",
      },
      size: {
        default: "px-4 py-2.5 text-xs",
        sm: "px-3 py-2 text-2xs",
        lg: "py-3.5 px-4 text-xs",
        icon: "p-0.5",
        tag: "px-2 py-0.5 text-2xs",
      },
    },
    defaultVariants: {
      variant: "outline",
      size: "default",
    },
  },
);

function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  }) {
  const Comp = asChild ? Slot.Root : "button";
  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
