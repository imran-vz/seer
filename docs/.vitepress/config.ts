import { defineConfig } from "vitepress";

export default defineConfig({
	title: "Seer",
	description: "Media file manager & metadata editor",
	lang: "en-US",
	base: "/",
	lastUpdated: true,
	cleanUrls: true,

	head: [
		["link", { rel: "icon", href: "/seer-logo.png" }],
		["meta", { name: "theme-color", content: "#0a0a0b" }],
		[
			"meta",
			{
				name: "keywords",
				content:
					"media file manager, metadata editor, audio codec detection, video re-encoding, bitrate analysis, file organization, batch processing, ffmpeg, ffprobe, sqlite caching, desktop app, open source, free software, macOS, Windows, Linux, tauri, rust",
			},
		],
		["meta", { property: "og:type", content: "website" }],
		["meta", { property: "og:site_name", content: "Seer" }],
		["meta", { property: "og:image", content: "/seer-logo.png" }],
		[
			"meta",
			{ property: "og:image:alt", content: "Seer application logo" },
		],
	],

	themeConfig: {
		logo: "/seer-logo.png",
		siteTitle: "Seer",

		nav: [
			{ text: "Home", link: "/" },
			{ text: "Features", link: "/features" },
			{ text: "Getting Started", link: "/getting-started" },
			{ text: "Installation", link: "/installation" },
		],

		sidebar: [
			{
				text: "Introduction",
				items: [
					{ text: "What is Seer?", link: "/" },
					{ text: "Features", link: "/features" },
				],
			},
			{
				text: "Getting Started",
				items: [
					{ text: "Installation", link: "/installation" },
					{ text: "Quick Start", link: "/getting-started" },
				],
			},
		],

		socialLinks: [
			{ icon: "github", link: "https://github.com/imran-vz/seer" },
		],

		footer: {
			message: "Released under the MIT License.",
			copyright: "Built with VitePress",
		},

		search: {
			provider: "local",
		},
	},

	sitemap: {
		hostname: "https://seer.imran.codes",
	},
});
