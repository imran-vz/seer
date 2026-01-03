import { CheckIcon, Monitor, Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

type Theme = "system" | "light" | "dark";

const themes: { value: Theme; label: string; icon: React.ReactNode }[] = [
	{ value: "system", label: "System", icon: <Monitor className="h-4 w-4" /> },
	{ value: "light", label: "Light", icon: <Sun className="h-4 w-4" /> },
	{ value: "dark", label: "Dark", icon: <Moon className="h-4 w-4" /> },
];

function getSystemTheme(): "light" | "dark" {
	return window.matchMedia("(prefers-color-scheme: dark)").matches
		? "dark"
		: "light";
}

function applyTheme(theme: Theme) {
	const resolved = theme === "system" ? getSystemTheme() : theme;
	document.documentElement.classList.add("no-transitions");
	document.documentElement.classList.remove("light", "dark");
	document.documentElement.classList.add(resolved);
	requestAnimationFrame(() => {
		requestAnimationFrame(() => {
			document.documentElement.classList.remove("no-transitions");
		});
	});
}

export function ThemeSwitcher() {
	const [theme, setTheme] = useState<Theme>(() => {
		return (localStorage.getItem("theme") as Theme) || "system";
	});

	useEffect(() => {
		applyTheme(theme);
		localStorage.setItem("theme", theme);
	}, [theme]);

	useEffect(() => {
		if (theme !== "system") return;
		const media = window.matchMedia("(prefers-color-scheme: dark)");
		const handler = () => applyTheme("system");
		media.addEventListener("change", handler);
		return () => media.removeEventListener("change", handler);
	}, [theme]);

	const currentTheme = themes.find((t) => t.value === theme) || themes[0];

	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<Button variant="ghost" size="icon-sm" className="shrink-0">
					{currentTheme.icon}
				</Button>
			</DropdownMenuTrigger>
			<DropdownMenuContent align="end">
				{themes.map((t) => (
					<DropdownMenuItem key={t.value} onClick={() => setTheme(t.value)}>
						{t.icon}
						{t.label}
						{t.value === theme ? <CheckIcon /> : null}
					</DropdownMenuItem>
				))}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
