import { PanelRightClose, PanelRightOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { useSettingsStore } from "@/stores/settingsStore";

export function RightPanelToggle() {
	const rightPanelVisible = useSettingsStore(
		(state) => state.settings.rightPanelVisible,
	);
	const updateSetting = useSettingsStore((state) => state.updateSetting);

	const togglePanel = () => {
		updateSetting("rightPanelVisible", !rightPanelVisible);
	};

	return (
		<Tooltip>
			<TooltipTrigger asChild>
				<Button
					variant="ghost"
					size="icon-sm"
					className="shrink-0"
					onClick={togglePanel}
				>
					{rightPanelVisible ? (
						<PanelRightClose className="h-4 w-4" />
					) : (
						<PanelRightOpen className="h-4 w-4" />
					)}
				</Button>
			</TooltipTrigger>
			<TooltipContent side="bottom">
				{rightPanelVisible ? "Hide right panel" : "Show right panel"}
			</TooltipContent>
		</Tooltip>
	);
}
