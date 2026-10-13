import { useEffect, useState } from "react";

export type Theme = "dark" | "light";

const readTheme = (): Theme =>
	document.documentElement.classList.contains("dark") ? "dark" : "light";

/**
 * The active theme, following the class App puts on <html>. Canvas and WebGL
 * plots bake colours in when they draw, so they need the value, not just CSS.
 */
export const useTheme = (): Theme => {
	const [theme, setTheme] = useState<Theme>(readTheme);

	useEffect(() => {
		const observer = new MutationObserver(() => setTheme(readTheme()));
		observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
		return () => observer.disconnect();
	}, []);

	return theme;
};
