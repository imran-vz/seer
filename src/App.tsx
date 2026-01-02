import { useState } from "react";
import { DependencyCheck } from "./components/DependencyCheck";
import { FileBrowser } from "./components/FileBrowser";
import { MetadataPanel } from "./components/MetadataPanel";
import { TitleBar } from "./components/TitleBar";

function App() {
	const [selectedFile, setSelectedFile] = useState<string | null>(null);
	const [depsChecked, setDepsChecked] = useState(false);

	if (!depsChecked) {
		return (
			<div className="flex flex-col h-screen overflow-hidden">
				<TitleBar />
				<DependencyCheck onComplete={() => setDepsChecked(true)} />
			</div>
		);
	}

	return (
		<div className="flex flex-col h-screen overflow-hidden">
			<TitleBar />
			<main className="flex flex-1 overflow-hidden">
				<div className="w-[55%] min-w-75 overflow-hidden">
					<FileBrowser onFileSelect={setSelectedFile} />
				</div>
				<div className="flex-1 bg-background overflow-hidden">
					<MetadataPanel filePath={selectedFile} />
				</div>
			</main>
		</div>
	);
}

export default App;
