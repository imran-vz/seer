import { useState } from "react";
import { Toaster } from "@/components/ui/sonner";
import { DependencyCheck } from "./components/DependencyCheck";
import { FileBrowser } from "./components/FileBrowser";
import { MetadataPanel } from "./components/MetadataPanel";
import { StreamsPanel } from "./components/StreamsPanel";
import { TitleBar } from "./components/TitleBar";
import { cn } from "./lib/utils";
import { useFileBrowserStore } from "./stores/fileBrowserStore";

type Tab = "metadata" | "streams";

function App() {
	const [depsChecked, setDepsChecked] = useState(false);
	const [activeTab, setActiveTab] = useState<Tab>("metadata");
	const selectedPath = useFileBrowserStore((state) => state.selectedPath);

	if (!depsChecked) {
		return (
			<div className="app-container flex h-screen flex-col overflow-hidden">
				<TitleBar />
				<DependencyCheck onComplete={() => setDepsChecked(true)} />
				<Toaster richColors />
			</div>
		);
	}

	return (
		<div className="app-container flex h-screen flex-col overflow-hidden">
			<TitleBar />
			<main className="flex flex-1 overflow-hidden">
				{/* File Browser Panel */}
				<div className="w-[55%] min-w-75 overflow-hidden">
					<FileBrowser />
				</div>

				{/* Right Panel */}
				<div className="flex flex-1 flex-col overflow-hidden border-border/50 border-l bg-background">
					{/* Tab Bar */}
					<div className="flex items-center gap-1 border-border/50 border-b bg-muted/30 px-2 py-1.5">
						<button
							type="button"
							onClick={() => setActiveTab("metadata")}
							className={cn(
								"rounded-md px-3 py-1 font-medium text-xs transition-colors",
								activeTab === "metadata"
									? "border border-border/50 bg-background text-foreground shadow-sm"
									: "text-muted-foreground hover:bg-background/50 hover:text-foreground",
							)}
						>
							Metadata
						</button>
						<button
							type="button"
							onClick={() => setActiveTab("streams")}
							className={cn(
								"rounded-md px-3 py-1 font-medium text-xs transition-colors",
								activeTab === "streams"
									? "border border-border/50 bg-background text-foreground shadow-sm"
									: "text-muted-foreground hover:bg-background/50 hover:text-foreground",
							)}
						>
							Streams
						</button>
					</div>

					{/* Panel Content */}
					<div className="flex-1 overflow-hidden">
						{activeTab === "metadata" ? (
							<MetadataPanel filePath={selectedPath} />
						) : (
							<StreamsPanel filePath={selectedPath} />
						)}
					</div>
				</div>
			</main>
			<Toaster richColors />
		</div>
	);
}

export default App;
