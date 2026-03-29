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
  const selectedValue = options.some((option) => option.value === value)
    ? value
    : options[0]?.value ?? "";

  return (
    <div className="relative w-full text-xs">
      <select
        value={selectedValue}
        onChange={(event) => onChange(event.target.value)}
        className="focus-ring w-full cursor-pointer appearance-none rounded border border-white/10 bg-black/40 p-2 pr-8 text-white transition-colors hover:border-white/20 focus-visible:border-primary"
      >
        {options.map((option) => (
          <option key={option.value} value={option.value} className="bg-[#1a1a1a] text-white">
            {option.label}
          </option>
        ))}
      </select>
      <ChevronDown
        aria-hidden="true"
        size={14}
        className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-slate-400"
      />
    </div>
  );
}
