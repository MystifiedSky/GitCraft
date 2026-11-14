import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

// shadcn-style button with the app's button kinds.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[4px] text-[13px] transition-colors disabled:opacity-40 disabled:pointer-events-none outline-none focus-visible:ring-1 focus-visible:ring-[#80B1EA]",
  {
    variants: {
      variant: {
        default: "bg-white/5 border border-white/10 text-fg hover:bg-white/10 hover:text-white",
        primary: "bg-accent/20 border border-accent/60 text-white hover:bg-accent/30",
        success: "bg-good/15 border border-good text-white hover:bg-good/25",
        danger: "bg-transparent border border-bad text-[#ef7a76] hover:bg-bad/10",
        ai: "bg-ai/15 border border-ai text-white hover:bg-ai/25",
        ghost: "bg-transparent text-fg hover:bg-white/8 hover:text-white",
      },
      size: {
        sm: "h-6 px-2 text-[12px]",
        md: "h-7 px-3",
        lg: "h-10 px-4 text-[14px]",
        icon: "h-7 w-7 p-0",
      },
    },
    defaultVariants: { variant: "default", size: "md" },
  },
);

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(({ className, variant, size, ...props }, ref) => (
  <button ref={ref} className={cn(buttonVariants({ variant, size }), className)} {...props} />
));
Button.displayName = "Button";
