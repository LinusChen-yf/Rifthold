import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  memo,
  type SVGProps,
} from "react";
import { invoke } from "@tauri-apps/api/core";

type ColorMode = "system" | "light" | "dark";

function useColorMode() {
  const [mode, setMode] = useState<ColorMode>(() => {
    return (localStorage.getItem("colorMode") as ColorMode) || "system";
  });

  useEffect(() => {
    const root = document.documentElement;
    const apply = (m: ColorMode) => {
      const isDark = m === "system"
        ? window.matchMedia("(prefers-color-scheme: dark)").matches
        : m === "dark";
      root.classList.toggle("dark", isDark);
    };

    apply(mode);
    localStorage.setItem("colorMode", mode);

    if (mode === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => apply("system");
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }
  }, [mode]);

  return [mode, setMode] as const;
}

function useTheme() {
  const [themeName, setThemeName] = useState(() => localStorage.getItem("themeName") || "claude");
  const [themeList, setThemeList] = useState<string[]>(() => {
    const saved = localStorage.getItem("themeList");
    return saved ? JSON.parse(saved) : ["claude"];
  });
  const [themeStyle, setThemeStyle] = useState<HTMLStyleElement | null>(null);

  const loadTheme = useCallback(async (name: string) => {
    try {
      const res = await fetch(`https://tweakcn.com/r/themes/${name}.json`);
      if (!res.ok) throw new Error("Theme not found");
      const data = await res.json();

      const cssContent = data.files?.find((f: { path: string; content: string }) => f.path.endsWith(".css"))?.content;
      if (!cssContent) return;

      if (themeStyle) themeStyle.remove();

      const style = document.createElement("style");
      style.id = "tweakcn-theme";
      style.textContent = cssContent;
      document.head.appendChild(style);
      setThemeStyle(style);

      localStorage.setItem("themeName", name);
      setThemeName(name);

      // Add to list if not exists
      setThemeList(prev => {
        if (prev.includes(name)) return prev;
        const newList = [name, ...prev];
        localStorage.setItem("themeList", JSON.stringify(newList));
        return newList;
      });
    } catch (e) {
      console.warn("Failed to load theme:", e);
    }
  }, [themeStyle]);

  return { themeName, themeList, loadTheme };
}

import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Log to terminal (useful for debugging in Tauri dev mode)
const log = (msg: string) => invoke("log_debug", { msg });

function SettingsIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" {...props}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 1v6m0 6v6M5.6 5.6l4.2 4.2m4.2 4.2l4.2 4.2M1 12h6m6 0h6M5.6 18.4l4.2-4.2m4.2-4.2l4.2-4.2" />
    </svg>
  );
}


function ColorModeToggle({ colorMode, setColorMode }: { colorMode: ColorMode; setColorMode: (m: ColorMode) => void }) {
  const [open, setOpen] = useState(false);
  const labels = { system: "System", light: "Light", dark: "Dark" };

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 rounded-full bg-muted px-3 py-2 text-xs text-foreground transition hover:bg-accent"
      >
        {labels[colorMode]}
        <svg className="h-3 w-3" viewBox="0 0 12 12" fill="currentColor"><path d="M3 5l3 3 3-3" /></svg>
      </button>
      {open && (
        <>
          <div className="fixed inset-0" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full mt-1 z-50 rounded-lg border border-border bg-popover shadow-lg overflow-hidden">
            {(["system", "light", "dark"] as const).map((m) => (
              <button
                key={m}
                onClick={() => { setColorMode(m); setOpen(false); }}
                className={`block w-full px-4 py-2 text-left text-sm hover:bg-accent ${colorMode === m ? "text-primary font-medium" : "text-foreground"}`}
              >
                {labels[m]}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

type WindowInfo = {
  id: string;
  title: string;
  appName: string;
  isTitleFallback?: boolean;
  thumbnail?: string;
};

const MOCK_WINDOWS: WindowInfo[] = [
  { id: "1", title: "Design review — overview overlay", appName: "Figma" },
  { id: "2", title: "Docs — Tauri command bridge", appName: "Arc" },
  { id: "3", title: "Terminal — cargo watch", appName: "WezTerm" },
  { id: "4", title: "Messaging: Product team", appName: "Linear" },
  { id: "5", title: "Landing copy draft v3", appName: "Notion" },
  { id: "6", title: "Repo: rifthold (main)", appName: "VS Code" },
  { id: "7", title: "System Monitor", appName: "iStat Menus", isTitleFallback: true },
  { id: "8", title: "Spreadsheet — metrics wk12", appName: "Numbers" },
  { id: "9", title: "Prototype — window previews", appName: "Framer" },
  { id: "10", title: "Mail — triage inbox", appName: "Hey" },
  { id: "11", title: "Music — focus mix", appName: "Spotify" },
  { id: "12", title: "Calendar — interviews", appName: "Cron", isTitleFallback: true },
];

const PREVIEW_GRADIENTS = [
  "linear-gradient(135deg, #0ea5e9 0%, #22d3ee 35%, #1d4ed8 100%)",
  "linear-gradient(135deg, #a855f7 0%, #6366f1 50%, #0ea5e9 100%)",
  "linear-gradient(135deg, #f59e0b 0%, #ef4444 45%, #ec4899 100%)",
  "linear-gradient(135deg, #10b981 0%, #22c55e 50%, #0ea5e9 100%)",
  "linear-gradient(135deg, #6366f1 0%, #312e81 45%, #0f172a 100%)",
];

const gradientForIndex = (index: number) =>
  PREVIEW_GRADIENTS[index % PREVIEW_GRADIENTS.length];

type WindowCardProps = {
  windowInfo: WindowInfo;
  selected: boolean;
  index: number;
  onSelect: () => void;
  onActivate: () => void;
};

const WindowCard = memo(function WindowCard({
  windowInfo,
  selected,
  index,
  onSelect,
  onActivate,
}: WindowCardProps) {
  const displayTitle = windowInfo.title || windowInfo.appName;
  const gradient = gradientForIndex(index);
  const hasThumbnail = !!windowInfo.thumbnail;

  return (
    <button
      type="button"
      onClick={onSelect}
      onDoubleClick={onActivate}
      className={`group flex h-full flex-col overflow-hidden rounded-2xl border bg-card transition duration-150 ease-out ${
        selected
          ? "border-primary shadow-lg ring-1 ring-ring"
          : "border-border hover:border-primary/50 hover:bg-accent"
      } focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring`}
      aria-pressed={selected}
    >
      <div
        className="relative aspect-video w-full overflow-hidden rounded-xl bg-muted"
        style={hasThumbnail ? {} : { backgroundImage: gradient }}
      >
        {hasThumbnail ? (
          <img
            src={windowInfo.thumbnail}
            alt={displayTitle}
            className="h-full w-full object-cover animate-fade-in"
            style={{
              animation: "fadeIn 0.3s ease-in-out"
            }}
          />
        ) : (
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_20%_20%,rgba(255,255,255,0.18),transparent_40%)] opacity-70" />
        )}
        <div className="absolute inset-x-4 top-4 flex items-center justify-between text-xs text-white/80">
          <span className="rounded-full border border-white/30 bg-black/25 px-2 py-1 uppercase tracking-[0.14em]">
            {windowInfo.appName}
          </span>
          {selected && (
            <span className="rounded-full bg-primary px-2 py-1 text-[11px] font-semibold text-primary-foreground">
              Selected
            </span>
          )}
        </div>
      </div>
      <div className="flex flex-1 flex-col justify-between gap-2 p-4 text-left">
        <div className="space-y-1">
          <p className="text-[11px] uppercase tracking-[0.14em] text-muted-foreground">
            Window
          </p>
          <p
            className="text-base font-semibold leading-snug text-foreground"
            style={{
              display: "-webkit-box",
              WebkitLineClamp: 2,
              WebkitBoxOrient: "vertical",
              overflow: "hidden",
            }}
          >
            {displayTitle}
          </p>
          <p className="truncate text-sm text-muted-foreground">
            {windowInfo.appName}
          </p>
        </div>
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span className="rounded-full bg-muted px-2 py-1">#{index + 1}</span>
          <span className="hidden items-center gap-1 rounded-full border border-border px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.16em] sm:flex">
            Enter to switch
          </span>
        </div>
      </div>
    </button>
  );
});

const CACHE_KEY = "rifthold_windows_cache";

function App() {
  const [colorMode, setColorMode] = useColorMode();
  const { themeName, themeList, loadTheme } = useTheme();

  // Load from cache on mount
  const loadFromCache = useCallback(() => {
    try {
      const cached = localStorage.getItem(CACHE_KEY);
      if (cached) {
        const parsed = JSON.parse(cached);
        console.log("[cache] loaded", parsed.length, "windows from cache");
        return parsed as WindowInfo[];
      }
    } catch (error) {
      console.warn("[cache] failed to load", error);
    }
    return MOCK_WINDOWS;
  }, []);

  const [windows, setWindows] = useState<WindowInfo[]>(loadFromCache);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [loadingThumbnails, setLoadingThumbnails] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [shortcut, setShortcut] = useState("alt+space");
  const [editingShortcut, setEditingShortcut] = useState("");
  const [hasScreenRecordingPermission, setHasScreenRecordingPermission] = useState(true);
  const [showHelp, setShowHelp] = useState(false);
  const [disableIME, setDisableIME] = useState(() => localStorage.getItem("disableIME") === "true");
  const searchRef = useRef<HTMLInputElement>(null);

  // Apply IME setting immediately when changed
  useEffect(() => {
    if (disableIME) {
      invoke("switch_to_english_input").catch(console.warn);
    }
  }, [disableIME]);
  const isComposingRef = useRef(false);

  // Save to cache whenever windows update
  useEffect(() => {
    try {
      localStorage.setItem(CACHE_KEY, JSON.stringify(windows));
      console.log("[cache] saved", windows.length, "windows to cache");
    } catch (error) {
      console.warn("[cache] failed to save", error);
    }
  }, [windows]);

  const resetOverlayState = useCallback(() => {
    setQuery("");
    setSelectedIndex(0);
    // Delay focus to avoid blocking
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        searchRef.current?.focus();
      });
    });
    // Switch to English input if setting is enabled
    if (localStorage.getItem("disableIME") === "true") {
      invoke("switch_to_english_input").catch(console.warn);
    }
  }, []);

  const normalizedQuery = query.trim().toLowerCase();
  const filteredWindows = useMemo(() => {
    if (!normalizedQuery) return windows;
    const terms = normalizedQuery.split(/\s+/);
    return windows.filter((windowInfo) => {
      const title = windowInfo.title.toLowerCase();
      const app = windowInfo.appName.toLowerCase();
      return terms.every(term => app.includes(term) || title.includes(term));
    });
  }, [normalizedQuery, windows]);


  useEffect(() => {
    searchRef.current?.focus();

    invoke<string>("get_shortcut").then(setShortcut).catch(console.error);
    invoke<boolean>("check_screen_recording_permission").then(setHasScreenRecordingPermission).catch(console.error);

    // Listen for window list updates from backend
    const setupListeners = async () => {
      // Listen for window list
      const unlistenList = await listen<WindowInfo[]>("windows:list", (event) => {
        console.log("[event] received window list:", event.payload.length, "windows");

        // Merge with existing thumbnails from cache
        setWindows(prev => {
          const newWindows = event.payload.map(newWindow => {
            // Find existing window with same id
            const existing = prev.find(w => w.id === newWindow.id);
            // Keep existing thumbnail if new window doesn't have one
            return {
              ...newWindow,
              thumbnail: newWindow.thumbnail || existing?.thumbnail
            };
          });

          console.log("[event] merged windows, with thumbnails:", newWindows.filter(w => w.thumbnail).length);
          return newWindows;
        });

        setIsLoading(false);
      });

      // Listen for individual thumbnails
      const unlistenThumbnail = await listen<{ id: string; thumbnail: string }>("window:thumbnail", (event) => {
        console.log("[event] received thumbnail for window:", event.payload.id);
        setWindows(prev =>
          prev.map(w =>
            w.id === event.payload.id ? { ...w, thumbnail: event.payload.thumbnail } : w
          )
        );
      });

      // Listen for thumbnails complete
      const unlistenComplete = await listen("windows:thumbnails-complete", () => {
        console.log("[event] all thumbnails loaded");
        setLoadingThumbnails(false);
      });

      // Trigger initial load (non-blocking)
      console.log("[mount] triggering background refresh");
      invoke("refresh_windows_async").catch(error => {
        console.warn("[mount] refresh failed", error);
      });

      return () => {
        unlistenList();
        unlistenThumbnail();
        unlistenComplete();
      };
    };

    setupListeners();
  }, []);

  useEffect(() => {
    let unlistenShow: UnlistenFn | undefined;

    const attach = async () => {
      try {
        unlistenShow = await listen("overview:show", () => {
          console.log("[overview:show] window shown - displaying cached content instantly!");

          resetOverlayState();

          // Trigger async refresh in background (non-blocking, updates via events)
          console.log("[overview:show] triggering background refresh...");
          setLoadingThumbnails(true);
          invoke("refresh_windows_async").catch(error => {
            console.warn("[overview:show] refresh failed", error);
            setLoadingThumbnails(false);
          });
        });
      } catch (error) {
        console.warn("listen overview:show failed", error);
      }
    };

    void attach();

    return () => {
      if (unlistenShow) {
        try {
          unlistenShow();
        } catch (error) {
          console.warn("unlisten overview:show failed", error);
        }
      }
    };
  }, [resetOverlayState]);

  useEffect(() => {
    if (filteredWindows.length === 0) {
      setSelectedIndex(-1);
      return;
    }
    setSelectedIndex((current) => {
      if (current < 0) return 0;
      if (current >= filteredWindows.length) return filteredWindows.length - 1;
      return current;
    });
  }, [filteredWindows.length]);

  const moveSelection = useCallback(
    (delta: number) => {
      setSelectedIndex((current) => {
        if (filteredWindows.length === 0) return -1;
        const next =
          current < 0
            ? 0
            : (current + delta + filteredWindows.length) %
              filteredWindows.length;
        return next;
      });
    },
    [filteredWindows.length],
  );

  const hideOverlay = useCallback(async () => {
    try {
      const windowInstance = getCurrentWindow();
      await windowInstance.hide();
    } catch (error) {
      console.warn("hide window failed", error);
    }
  }, []);

  const activateWindow = useCallback(
    async (target?: WindowInfo) => {
      if (!target) return;
      console.log(`activate window id=${target.id}`);
      try {
        await invoke("activate_window", { id: target.id });
      } catch (error) {
        console.warn("activate_window failed, mock only", error);
      } finally {
        resetOverlayState();
        hideOverlay();
      }
    },
    [hideOverlay, resetOverlayState],
  );

  const activateSelected = useCallback(() => {
    const target = filteredWindows[selectedIndex] ?? filteredWindows[0];
    if (!target) return;
    activateWindow(target);
  }, [activateWindow, filteredWindows, selectedIndex]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      // Handle Ctrl + arrow keys or Ctrl + hjkl for grid navigation
      if (event.ctrlKey && !event.metaKey) {
        switch (event.key) {
          case "ArrowLeft":
          case "h":
            event.preventDefault();
            moveSelection(-1);
            break;
          case "ArrowDown":
          case "j":
            event.preventDefault();
            moveSelection(4);
            break;
          case "ArrowUp":
          case "k":
            event.preventDefault();
            moveSelection(-4);
            break;
          case "ArrowRight":
          case "l":
            event.preventDefault();
            moveSelection(1);
            break;
        }
        return;
      }

      // Skip other modifier keys
      if (event.metaKey || event.ctrlKey) return;

      switch (event.key) {
        case "Enter":
          if (event.isComposing || event.keyCode === 229 || isComposingRef.current) return;
          event.preventDefault();
          activateSelected();
          break;
        case "Escape":
          event.preventDefault();
          hideOverlay();
          break;
        default:
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activateSelected, hideOverlay, moveSelection]);

  const headline =
    filteredWindows.length === windows.length && !normalizedQuery
      ? "All windows at a glance"
      : `Filtered ${filteredWindows.length} of ${windows.length}`;

  return (
    <div className="relative h-screen overflow-y-auto bg-background text-foreground">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_20%_20%,rgba(56,189,248,0.12),transparent_35%)]" />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_80%_0%,rgba(99,102,241,0.1),transparent_35%)]" />

      <div className="relative mx-auto flex max-w-6xl flex-col gap-6 px-6 pb-10 pt-10">
        <header className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <p className="text-lg font-semibold uppercase tracking-[0.12em] text-primary">
            Rifthold overview
          </p>
          <div className="flex items-center gap-2">
            <ColorModeToggle colorMode={colorMode} setColorMode={setColorMode} />
            <button
              type="button"
              onClick={() => setShowHelp(true)}
              className="rounded-full bg-muted p-2 transition hover:bg-accent"
              title="Help"
            >
              <svg className="h-4 w-4 text-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10" />
                <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
                <circle cx="12" cy="17" r="0.5" fill="currentColor" />
              </svg>
            </button>
            <button
              type="button"
              onClick={() => {
                setEditingShortcut(shortcut);
                setShowSettings(true);
              }}
              className="rounded-full bg-muted p-2 transition hover:bg-accent"
              title="Settings"
            >
              <SettingsIcon className="h-4 w-4 text-foreground" />
            </button>
          </div>
        </header>

        {!hasScreenRecordingPermission && (
          <div className="flex items-start gap-3 rounded-2xl border border-amber-400/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-100">
            <div className="mt-[2px] h-2 w-2 rounded-full bg-amber-300" />
            <div className="space-y-1">
              <p className="font-semibold">Screen Recording permission required</p>
              <p className="text-amber-100/80">
                Grant <strong>Screen Recording</strong> permission to Rifthold in System Settings → Privacy & Security → Screen Recording to see window titles and thumbnails.
              </p>
            </div>
          </div>
        )}

        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-3 rounded-2xl border border-border bg-card px-4 py-3 shadow-lg focus-within:border-ring">
            <SearchIcon className="h-5 w-5 text-muted-foreground" />
            <input
              ref={searchRef}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onCompositionStart={() => { isComposingRef.current = true; }}
              onCompositionEnd={() => { isComposingRef.current = false; }}
              placeholder="Search by window title or app…"
              className="w-full bg-transparent text-base text-foreground placeholder:text-muted-foreground outline-none"
            />
            <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground whitespace-nowrap">
              {filteredWindows.length} / {windows.length}
            </span>
            <button
              type="button"
              onClick={() => {
                setLoadingThumbnails(true);
                invoke("refresh_windows_async").catch(error => {
                  console.warn("refresh failed", error);
                  setLoadingThumbnails(false);
                });
              }}
              className="flex items-center gap-1.5 rounded-full bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              title="Refresh with thumbnails"
            >
              <svg className="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2" />
              </svg>
              Refresh
            </button>
          </div>
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>{headline}</span>
            {isLoading && <span>Loading windows…</span>}
            {loadingThumbnails && <span>Loading thumbnails…</span>}
          </div>
        </div>

        <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {filteredWindows.map((windowInfo, index) => {
            const isSelected = index === selectedIndex;
            return (
              <WindowCard
                key={windowInfo.id}
                windowInfo={windowInfo}
                selected={isSelected}
                index={index}
                onSelect={() => setSelectedIndex(index)}
                onActivate={() => activateWindow(windowInfo)}
              />
            );
          })}
        </section>

        {filteredWindows.length === 0 && (
          <div className="rounded-2xl border border-border bg-card px-6 py-10 text-center text-muted-foreground">
            <p className="text-lg font-semibold text-foreground">No windows match</p>
            <p className="text-sm">Try a different keyword or clear the search box.</p>
          </div>
        )}
      </div>

      {showSettings && (
        <div className="fixed inset-0 flex items-center justify-center bg-black/50" onClick={() => setShowSettings(false)}>
          <div className="w-96 rounded-2xl border border-border bg-popover p-6" onClick={(e) => e.stopPropagation()}>
            <h2 className="mb-4 text-xl font-semibold text-foreground">Settings</h2>
            <div className="space-y-4">
              <div>
                <label className="mb-2 block text-sm text-muted-foreground">Global Shortcut</label>
                <input
                  type="text"
                  value={editingShortcut}
                  onChange={(e) => setEditingShortcut(e.target.value)}
                  placeholder="e.g., alt+space, cmd+shift+o"
                  className="w-full rounded-lg border border-input bg-background px-3 py-2 text-foreground outline-none focus:border-ring"
                />
                <p className="mt-1 text-xs text-muted-foreground">Examples: alt+space, cmd+shift+o, ctrl+`</p>
              </div>
              <div>
                <label className="flex items-center gap-2 text-sm text-muted-foreground cursor-pointer">
                  <input
                    type="checkbox"
                    checked={disableIME}
                    onChange={(e) => {
                      setDisableIME(e.target.checked);
                      localStorage.setItem("disableIME", String(e.target.checked));
                    }}
                    className="h-4 w-4 rounded border-input accent-primary"
                  />
                  Use English Input
                </label>
              </div>
              <div>
                <label className="mb-2 block text-sm text-muted-foreground">Theme</label>
                <div className="flex flex-wrap gap-1 mb-2">
                  {themeList.map((t) => (
                    <button
                      key={t}
                      onClick={() => loadTheme(t)}
                      className={`px-2 py-1.5 text-xs rounded transition ${themeName === t ? "bg-primary text-primary-foreground" : "bg-muted hover:bg-accent text-foreground"}`}
                    >
                      {t}
                    </button>
                  ))}
                </div>
                <input
                  type="text"
                  placeholder="Enter theme name..."
                  className="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-ring"
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      loadTheme((e.target as HTMLInputElement).value);
                      (e.target as HTMLInputElement).value = "";
                    }
                  }}
                />
                <button
                  type="button"
                  onClick={async () => {
                    setShowSettings(false);
                    try {
                      const { open } = await import("@tauri-apps/plugin-shell");
                      await open("https://tweakcn.com/editor/theme");
                      await hideOverlay();
                    } catch (err) {
                      console.error("[browse] error:", err);
                    }
                  }}
                  className="mt-2 w-full rounded-lg border border-border bg-muted px-3 py-2 text-xs text-muted-foreground hover:bg-accent"
                >
                  Browse themes at tweakcn.com
                </button>
              </div>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={async () => {
                    try {
                      await invoke("set_shortcut", { shortcut: editingShortcut });
                      setShortcut(editingShortcut);
                      setShowSettings(false);
                    } catch (error) {
                      alert(`Failed to set shortcut: ${error}`);
                    }
                  }}
                  className="flex-1 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition hover:opacity-90"
                >
                  Save
                </button>
                <button
                  type="button"
                  onClick={() => setShowSettings(false)}
                  className="flex-1 rounded-lg border border-border bg-muted px-4 py-2 text-sm font-semibold text-foreground transition hover:bg-accent"
                >
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {showHelp && (
        <div className="fixed inset-0 flex items-center justify-center bg-black/50" onClick={() => setShowHelp(false)}>
          <div className="w-[28rem] rounded-2xl border border-border bg-popover p-6" onClick={(e) => e.stopPropagation()}>
            <h2 className="mb-4 text-xl font-semibold text-foreground">Help</h2>
            <div className="space-y-4 text-sm text-muted-foreground">
              <div>
                <h3 className="mb-2 font-semibold text-foreground">Window Navigation</h3>
                <ul className="list-inside list-disc space-y-1">
                  <li>Ctrl + ↑↓←→ or Ctrl + hjkl to navigate</li>
                  <li>Enter to activate selected window</li>
                  <li>Esc to hide overlay</li>
                </ul>
              </div>
              <div>
                <h3 className="mb-2 font-semibold text-foreground">Search</h3>
                <ul className="list-inside list-disc space-y-1">
                  <li>Type to filter by app name or window title</li>
                  <li>Use spaces to separate multiple keywords</li>
                  <li>E.g. <code className="rounded bg-accent px-1">code vib</code> matches windows where app contains "code" and title contains "vib"</li>
                </ul>
              </div>
            </div>
            <button
              type="button"
              onClick={() => setShowHelp(false)}
              className="mt-4 w-full rounded-lg border border-border bg-secondary px-4 py-2 text-sm font-semibold text-foreground transition hover:bg-accent"
            >
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function SearchIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      {...props}
    >
      <circle cx="11" cy="11" r="7" />
      <path d="m15.5 15.5 4 4" strokeLinecap="round" />
    </svg>
  );
}

export default App;
