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
		// biome-ignore lint/a11y/useSemanticElements: this is to support drags and window matching
		<header
			className="flex justify-between items-center gap-3 pl-20 pr-4 py-2 border-b border-border bg-card select-none w-full h-14"
			onMouseDown={handleMouseDown}
			onDoubleClick={handleDoubleClick}
			role="button"
		>
			<div className="grid grid-cols-2 place-items-center gap-0">
				<img data-no-drag src="/seer.svg" className="w-6 h-6" alt="Seer" />
				<h1 className="text-base font-semibold flex-1 leading-0">Seer</h1>
			</div>
			<div data-no-drag>
				<ThemeSwitcher />
			</div>
		</header>
	);
}
