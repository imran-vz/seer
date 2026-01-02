import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Apply saved theme or default to system preference
const savedTheme = localStorage.getItem("theme") || "system";
const resolvedTheme =
	savedTheme === "system"
		? window.matchMedia("(prefers-color-scheme: dark)").matches
			? "dark"
			: "light"
		: savedTheme;
document.documentElement.classList.add(resolvedTheme);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
	<React.StrictMode>
		<App />
	</React.StrictMode>,
);
