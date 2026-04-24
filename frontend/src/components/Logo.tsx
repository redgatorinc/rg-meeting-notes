import React from "react";
import Image from "next/image";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";

interface LogoProps {
    isCollapsed: boolean;
}

const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(({ isCollapsed }, ref) => {
  return (
    <Dialog aria-describedby={undefined}>
      {isCollapsed ? (
        <DialogTrigger asChild>
          <button
            ref={ref}
            className="flex items-center justify-center mb-2 cursor-pointer bg-transparent border-none p-0 hover:opacity-80 transition-opacity"
          >
            <Image
              src="/redgator-icon.png"
              alt="Redgator"
              width={32}
              height={32}
              priority
            />
          </button>
        </DialogTrigger>
      ) : (
        <DialogTrigger asChild>
          <button
            ref={ref}
            className="flex items-center justify-start mb-2 cursor-pointer bg-transparent border-none p-0 hover:opacity-80 transition-opacity w-full"
          >
            <Image
              src="/redgator-logo.png"
              alt="Redgator"
              width={180}
              height={40}
              priority
              style={{ height: 'auto', width: '100%', maxWidth: 180 }}
            />
          </button>
        </DialogTrigger>
      )}
      <DialogContent>
        <VisuallyHidden>
          <DialogTitle>About Meetily</DialogTitle>
        </VisuallyHidden>
        <About />
      </DialogContent>
    </Dialog>
  );
});

Logo.displayName = "Logo";

export default Logo;