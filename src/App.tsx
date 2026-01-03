import { useEffect, useState } from "react";
import { Toaster } from "@/components/ui/sonner";
import { BitratePanel } from "./components/BitratePanel";
import { DependencyCheck } from "./components/DependencyCheck";
import { FileBrowser } from "./components/FileBrowser";
import { MetadataPanel } from "./components/MetadataPanel";
import { StreamsPanel } from "./components/StreamsPanel";
import { TitleBar } from "./components/TitleBar";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import { isVideoOrAudioFile } from "./lib/fileUtils";
import { useInitDatabase } from "./lib/useDatabase";
import { useFileBrowserStore } from "./stores/fileBrowserStore";

function App() {
	const [depsChecked, setDepsChecked] = useState(false);
	const [activeTab, setActiveTab] = useState("metadata");
	const selectedPath = useFileBrowserStore((state) => state.selectedPath);
	const showBitrateTab = isVideoOrAudioFile(selectedPath);

	// Initialize SQLite database
	const { initialized: dbInitialized, error: dbError } = useInitDatabase();

	// Log database initialization status
	useEffect(() => {
		if (dbInitialized) {
			console.log("[App] Database initialized successfully");
		}
		if (dbError) {
			console.error("[App] Database initialization failed:", dbError);
		}
	}, [dbInitialized, dbError]);

	// Switch to metadata tab if user selects an image while on bitrate tab
	useEffect(() => {
		if (!showBitrateTab && activeTab === "bitrate") {
			setActiveTab("metadata");
		}
	}, [showBitrateTab, activeTab]);

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
				<Tabs
					value={activeTab}
					onValueChange={setActiveTab}
					className="flex flex-1 flex-col overflow-hidden border-border/50 border-l bg-background"
				>
					<div className="border-border/50 border-b bg-muted/30 px-2 py-1.5">
						<TabsList>
							<TabsTrigger value="metadata">Metadata</TabsTrigger>
							<TabsTrigger value="streams">Streams</TabsTrigger>
							{showBitrateTab && (
								<TabsTrigger value="bitrate">Bitrate</TabsTrigger>
							)}
						</TabsList>
					</div>

					<TabsContent value="metadata" className="flex-1 overflow-hidden">
						<MetadataPanel filePath={selectedPath} />
					</TabsContent>
					<TabsContent value="streams" className="flex-1 overflow-hidden">
						<StreamsPanel filePath={selectedPath} />
					</TabsContent>
					{showBitrateTab && (
						<TabsContent value="bitrate" className="flex-1 overflow-hidden">
							<BitratePanel filePath={selectedPath} />
						</TabsContent>
					)}
				</Tabs>
			</main>
			<Toaster richColors />
		</div>
	);
}

export default App;
