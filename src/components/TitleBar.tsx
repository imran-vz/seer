import { getCurrentWindow } from "@tauri-apps/api/window";
import { ThemeSwitcher } from "./ThemeSwitcher";

export function TitleBar() {
	const appWindow = getCurrentWindow();

	const handleMouseDown = async (e: React.MouseEvent) => {
		const target = e.target as HTMLElement;
		if (target.closest("[data-no-drag]")) return;
		if (e.buttons === 1 && e.detail === 1) {
			await appWindow.startDragging();
		}
	};

	const handleDoubleClick = async (e: React.MouseEvent) => {
		const target = e.target as HTMLElement;
		if (target.closest("[data-no-drag]")) return;
		await appWindow.toggleMaximize();
	};

	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: required for window dragging
		<header
			className="titlebar-drag-region flex h-12 select-none items-center border-border/50 border-b bg-muted/40 pr-3 pl-23"
			onMouseDown={handleMouseDown}
			onDoubleClick={handleDoubleClick}
		>
			{/* App Icon - inline with traffic lights (y=24) with 1rem margin */}
			<div
				data-no-drag
				className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/50 bg-background shadow-sm"
			>
				<img src="/seer.svg" className="h-4 w-4 dark:hidden" alt="Seer" />
				<img
					src="/seer-dark.svg"
					className="hidden h-4 w-4 dark:block"
					alt="Seer"
				/>
			</div>

			{/* Spacer */}
			<div className="flex-1" />

			{/* Right side controls */}
			<div data-no-drag>
				<ThemeSwitcher />
			</div>
		</header>
	);
}
