import * as React from "react";

interface SelectProps {
	value?: string;
	onValueChange?: (value: string) => void;
	children: React.ReactNode;
}

interface SelectTriggerProps {
	className?: string;
	children: React.ReactNode;
}

interface SelectContentProps {
	children: React.ReactNode;
}

interface SelectItemProps {
	value: string;
	children: React.ReactNode;
}

interface SelectValueProps {}

const SelectContext = React.createContext<{
	value?: string;
	onValueChange?: (value: string) => void;
}>({});

export function Select({ value, onValueChange, children }: SelectProps) {
	return (
		<SelectContext.Provider value={{ value, onValueChange }}>
			<div className="relative">{children}</div>
		</SelectContext.Provider>
	);
}

export function SelectTrigger({ className = "", children }: SelectTriggerProps) {
	const [open, setOpen] = React.useState(false);

	return (
		<button
			type="button"
			onClick={() => setOpen(!open)}
			className={`flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
		>
			{children}
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="24"
				height="24"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="2"
				strokeLinecap="round"
				strokeLinejoin="round"
				className="h-4 w-4 opacity-50"
			>
				<polyline points="6 9 12 15 18 9" />
			</svg>
		</button>
	);
}

export function SelectValue({}: SelectValueProps) {
	const { value } = React.useContext(SelectContext);
	return <span>{value || "Select..."}</span>;
}

export function SelectContent(_props: SelectContentProps) {
	// This is a simplified version - in production would use Radix UI
	// For now, we'll use a native select in the parent component
	return null;
}

export function SelectItem({ value, children }: SelectItemProps) {
	const { onValueChange } = React.useContext(SelectContext);

	return (
		<div
			onClick={() => onValueChange?.(value)}
			className="relative flex cursor-pointer select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none hover:bg-accent hover:text-accent-foreground"
		>
			{children}
		</div>
	);
}
