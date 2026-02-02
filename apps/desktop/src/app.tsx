import { useEffect, useState } from "react";
import { ThemeToggle } from "./components/ui/theme-toggle";

export const App = () => {
	const [theme, setTheme] = useState<"dark" | "light">(() => {
		if (typeof window !== "undefined") {
			const stored = localStorage.getItem("phonetic-theme");
			if (stored === "light" || stored === "dark") return stored;
			return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
		}
		return "dark";
	});

	useEffect(() => {
		document.documentElement.classList.remove("light", "dark");
		document.documentElement.classList.add(theme);
		localStorage.setItem("phonetic-theme", theme);
	}, [theme]);

	const toggleTheme = () => {
		setTheme((prev) => (prev === "dark" ? "light" : "dark"));
	};

	return (
		<div className="flex min-h-screen flex-col bg-background text-foreground">
			{/* Header */}
			<header className="glass sticky top-0 z-50 flex h-14 items-center justify-between border-b px-4">
				<h1 className="gradient-text text-xl font-bold tracking-tight">Phonetic</h1>
				<ThemeToggle theme={theme} onToggle={toggleTheme} />
			</header>

			{/* Main content */}
			<main className="flex flex-1 gap-4 p-4">
				{/* Sidebar - Device & Generator controls */}
				<aside className="glass w-72 flex-shrink-0 rounded-lg p-4">
					<h2 className="mb-4 text-sm font-semibold uppercase tracking-wider text-muted-foreground">
						Audio Devices
					</h2>
					<p className="text-sm text-muted-foreground">
						Connect an audio interface to begin measurement.
					</p>
				</aside>

				{/* Main visualization area */}
				<div className="flex flex-1 flex-col gap-4">
					{/* Spectrum analyzer placeholder */}
					<div className="glass flex-1 rounded-lg p-4">
						<h2 className="mb-4 text-sm font-semibold uppercase tracking-wider text-muted-foreground">
							Spectrum Analyzer
						</h2>
						<div className="flex h-64 items-center justify-center rounded-md border border-dashed border-border bg-background/50">
							<p className="text-muted-foreground">Spectrum visualization will appear here</p>
						</div>
					</div>

					{/* Transfer function placeholder */}
					<div className="glass flex-1 rounded-lg p-4">
						<h2 className="mb-4 text-sm font-semibold uppercase tracking-wider text-muted-foreground">
							Transfer Function
						</h2>
						<div className="flex h-64 items-center justify-center rounded-md border border-dashed border-border bg-background/50">
							<p className="text-muted-foreground">Transfer function display will appear here</p>
						</div>
					</div>
				</div>
			</main>

			{/* Footer status bar */}
			<footer className="glass flex h-8 items-center justify-between border-t px-4 text-xs text-muted-foreground">
				<span>Sample Rate: --</span>
				<span>Buffer: --</span>
				<span>Latency: --</span>
			</footer>
		</div>
	);
};
