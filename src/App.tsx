import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Toaster } from "@/components/ui/sonner";
import { BitratePanel } from "./components/BitratePanel";
import { DependencyCheck } from "./components/DependencyCheck";
import { FileBrowser } from "./components/FileBrowser";
import { MetadataPanel } from "./components/MetadataPanel";
import { StreamsPanel } from "./components/StreamsPanel";
import { TitleBar } from "./components/TitleBar";
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "./components/ui/resizable";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import { useAppInitialization } from "./hooks/useAppInitialization";
import { isVideoOrAudioFile } from "./lib/fileUtils";
import { useFileBrowserStore } from "./stores/fileBrowserStore";
import { useSettingsStore } from "./stores/settingsStore";

function LoadingScreen({
	stage,
	error,
}: {
	stage: string;
	error: string | null;
}) {
	const stageLabels: Record<string, string> = {
		database: "Initializing database...",
		settings: "Loading settings...",
		"file-browser": "Preparing file browser...",
	};

	return (
		<div className="app-container flex h-screen flex-col overflow-hidden">
			<TitleBar />
			<div className="flex flex-1 flex-col items-center justify-center gap-4">
				{error ? (
					<>
						<div className="font-medium text-destructive text-lg">
							Initialization Error
						</div>
						<div className="max-w-md text-center text-muted-foreground text-sm">
							{error}
						</div>
					</>
				) : (
					<>
						<Loader2 className="size-10 animate-spin text-primary" />
						<div className="text-muted-foreground text-sm">
							{stageLabels[stage] || "Loading..."}
						</div>
					</>
				)}
			</div>
			<Toaster richColors />
		</div>
	);
}

function App() {
	const [depsChecked, setDepsChecked] = useState(false);
	const [activeTab, setActiveTab] = useState("metadata");
	const selectedPath = useFileBrowserStore((state) => state.selectedPath);
	const showBitrateTab = isVideoOrAudioFile(selectedPath);
	const rightPanelVisible = useSettingsStore(
		(state) => state.settings.rightPanelVisible,
	);

	// Centralized app initialization
	const { stage, initialized, error } = useAppInitialization();

	// Switch to metadata tab if user selects an image while on bitrate tab
	useEffect(() => {
		if (!showBitrateTab && activeTab === "bitrate") {
			setActiveTab("metadata");
		}
	}, [showBitrateTab, activeTab]);

	// Show dependency check first
	if (!depsChecked) {
		return (
			<div className="app-container flex h-screen flex-col overflow-hidden">
				<TitleBar />
				<DependencyCheck onComplete={() => setDepsChecked(true)} />
				<Toaster richColors />
			</div>
		);
	}

	// Show loading screen while stores are initializing
	if (!initialized || error) {
		return <LoadingScreen stage={stage} error={error} />;
	}

	return (
		<div className="app-container flex h-screen flex-col overflow-hidden">
			<TitleBar />
			<main className="flex flex-1 overflow-hidden">
				<ResizablePanelGroup direction="horizontal">
					{/* File Browser Panel */}
					<ResizablePanel
						defaultSize={rightPanelVisible ? 55 : 100}
						minSize={30}
						className="overflow-hidden"
					>
						<FileBrowser />
					</ResizablePanel>

					{/* Right Panel */}
					{rightPanelVisible && (
						<>
							<ResizableHandle withHandle />
							<ResizablePanel defaultSize={45} minSize={30}>
								<Tabs
									value={activeTab}
									onValueChange={setActiveTab}
									className="flex h-full flex-col overflow-hidden border-border/50 border-l bg-background"
								>
									<div className="border-border/50 border-b bg-muted/30 px-2 py-1.5">
										<TabsList>
											<TabsTrigger value="metadata">Metadata</TabsTrigger>
											<TabsTrigger value="streams">Streams</TabsTrigger>
											<TabsTrigger disabled={!showBitrateTab} value="bitrate">
												Bitrate
											</TabsTrigger>
										</TabsList>
									</div>

									<TabsContent
										value="metadata"
										className="flex-1 overflow-hidden"
									>
										<MetadataPanel filePath={selectedPath} />
									</TabsContent>
									<TabsContent
										value="streams"
										className="flex-1 overflow-hidden"
									>
										<StreamsPanel filePath={selectedPath} />
									</TabsContent>
									{showBitrateTab && (
										<TabsContent
											value="bitrate"
											className="flex-1 overflow-hidden"
										>
											<BitratePanel filePath={selectedPath} />
										</TabsContent>
									)}
								</Tabs>
							</ResizablePanel>
						</>
					)}
				</ResizablePanelGroup>
			</main>
			<Toaster richColors />
		</div>
	);
}

export default App;
