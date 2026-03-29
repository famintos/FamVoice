import { useState, useRef, useEffect } from "react";
import { ChevronDown } from "lucide-react";

interface Option {
  value: string;
  label: string;
}

interface SelectProps {
  value: string;
  onChange: (value: string) => void;
  options: Option[];
}

export function Select({ value, onChange, options }: SelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const selectedOption = options.find((opt) => opt.value === value) || options[0];

  return (
    <div className="relative w-full text-xs" ref={containerRef}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`flex w-full cursor-pointer items-center justify-between rounded border bg-black/40 p-2 text-white transition-colors focus:outline-none ${
          isOpen ? "border-primary" : "border-white/10"
        }`}
      >
        <span className="truncate">{selectedOption?.label}</span>
        <ChevronDown size={14} className={`text-slate-400 transition-transform ${isOpen ? "rotate-180 text-primary" : ""}`} />
      </button>

      {isOpen && (
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded border border-white/10 bg-[#1a1a1a] shadow-lg">
          <div className="max-h-60 overflow-y-auto custom-scrollbar">
            {options.map((option) => (
              <button
                key={option.value}
                type="button"
                className={`w-full cursor-pointer px-3 py-2.5 text-left text-xs transition-colors ${
                  option.value === value
                    ? "bg-primary/20 text-primary"
                    : "text-slate-300 hover:bg-white/10 hover:text-white"
                }`}
                onClick={() => {
                  onChange(option.value);
                  setIsOpen(false);
                }}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
