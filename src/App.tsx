import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import DownloadItem from "./components/DownloadItem";
import "./App.css";

interface DownloadInfo {
  id: string;
  url: string;
  token: string;
  filename: string;
  save_dir: string;
  total_size: number;
  downloaded: number;
  status: string;
  error: string | null;
  created_at: string;
  file_missing: boolean;
}

interface ProgressEvent {
  id: string;
  downloaded: number;
  total: number;
  speed: number;
}

interface StatusEvent {
  id: string;
  status: string;
  error: string | null;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec === 0) return "--";
  return `${formatBytes(bytesPerSec)}/s`;
}

function parseClipboard(text: string): { url: string; token: string } | null {
  const trimmed = text.trim();
  const headerRe = /(?:--header|-H)\s*=\s*["']Authorization:\s*([^"']+)["']/;
  const headerMatch = trimmed.match(headerRe);
  if (!headerMatch) return null;
  const urlRe = /(https?:\/\/[^\s'"<>]+)/g;
  const urls = trimmed.match(urlRe);
  if (!urls || urls.length === 0) return null;
  return { token: headerMatch[1].trim(), url: urls[urls.length - 1] };
}

export default function App() {
  const [url, setUrl] = useState("");
  const [token, setToken] = useState("");
  const [saveDir, setSaveDir] = useState("");
  const [downloads, setDownloads] = useState<DownloadInfo[]>([]);
  const [speedMap, setSpeedMap] = useState<Record<string, number>>({});
  const [starting, setStarting] = useState(false);
  const [showTips, setShowTips] = useState(false);

  useEffect(() => {
    invoke<string>("get_default_download_dir").then(setSaveDir).catch(() => {});
    invoke<DownloadInfo[]>("list_downloads").then(setDownloads).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<ProgressEvent>("download-progress", (event) => {
      const { id, downloaded, total, speed } = event.payload;
      setSpeedMap((prev) => ({ ...prev, [id]: speed }));
      setDownloads((prev) =>
        prev.map((d) =>
          d.id === id
            ? {
                ...d,
                downloaded,
                total_size: total || d.total_size,
                status: downloaded >= total && total > 0 ? "completed" : d.status,
              }
            : d
        )
      );
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen for status-change events
  useEffect(() => {
    const unlisten = listen<StatusEvent>("download-status", (event) => {
      const { id, status, error } = event.payload;
      setDownloads((prev) =>
        prev.map((d) =>
          d.id === id
            ? { ...d, status, error }
            : d
        )
      );
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleBrowse = async () => {
    const selected = await open({ directory: true, title: "选择保存目录" });
    if (selected) setSaveDir(selected as string);
  };

  const handleUrlPaste = (e: React.ClipboardEvent<HTMLInputElement>) => {
    const pasted = e.clipboardData.getData("text");
    const parsed = parseClipboard(pasted);
    if (parsed) {
      e.preventDefault();
      setUrl(parsed.url);
      setToken(parsed.token);
    }
  };

  const handleStart = async () => {
    if (!url.trim() || !token.trim() || !saveDir.trim()) return;
    setStarting(true);
    try {
      const info = await invoke<DownloadInfo>("start_download", {
        url: url.trim(),
        token: token.trim(),
        saveDir: saveDir.trim(),
      });
      setDownloads((prev) => [...prev, info]);
      setUrl("");
      setToken("");
    } catch (e) {
      alert(String(e));
    } finally {
      setStarting(false);
    }
  };

  const handlePause = async (id: string) => {
    await invoke("pause_download", { id });
    setDownloads((prev) =>
      prev.map((d) => (d.id === id ? { ...d, status: "paused" } : d))
    );
  };

  const handleResume = async (id: string) => {
    await invoke("resume_download", { id });
    setDownloads((prev) =>
      prev.map((d) =>
        d.id === id ? { ...d, status: "downloading", error: null } : d
      )
    );
  };

  const handleCancel = async (id: string) => {
    await invoke("cancel_download", { id });
    setDownloads((prev) => prev.filter((d) => d.id !== id));
    setSpeedMap((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  };

  const handleRemove = async (id: string) => {
    await invoke("remove_download", { id });
    setDownloads((prev) => prev.filter((d) => d.id !== id));
    setSpeedMap((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  };

  const handleRedownload = async (id: string) => {
    try {
      const info = await invoke<DownloadInfo>("redownload_download", { id });
      setDownloads((prev) =>
        prev.map((d) =>
          d.id === id
            ? { ...d, downloaded: 0, total_size: 0, status: "downloading", error: null }
            : d
        )
      );
    } catch (e) {
      alert(String(e));
    }
  };

  const handleOpenDir = async (saveDir: string, filename: string) => {
    try {
      await invoke("open_download_dir", { saveDir, filename });
    } catch (e) {
      alert(String(e));
    }
  };

  return (
    <div className="app">
      <div className="header">
        <span className="logo">📦</span>
        <span>SMC 下载器</span>
        <button className="tips-btn" onClick={() => setShowTips((v) => !v)}>
          💡 使用技巧
        </button>
      </div>

      {showTips && (
        <div className="tips-panel">
          <div className="tips-panel-header">
            <span>💡 使用技巧</span>
            <button className="tips-close" onClick={() => setShowTips(false)}>✕</button>
          </div>
          <p className="tips-desc">
            直接将 <code>wget</code> 或 <code>curl</code> 下载命令粘贴到「下载链接」输入框，应用会自动解析并填充下载地址和 Token，无需手动拆分。
          </p>
          <div className="tips-example">
            <div className="tips-label">支持格式一（wget）：</div>
            <pre className="tips-code">wget --header="Authorization: 9a572070d7464c98b0ac4c473af59cc22f7880aca7654529a5d023fd6593c203" http://47.92.201.211:99/20260423-上海合合doris-202610121101.tar.gz</pre>
          </div>
          <div className="tips-example">
            <div className="tips-label">支持格式二（curl）：</div>
            <pre className="tips-code">curl -O -H "Authorization: 9a572070d7464c98b0ac4c473af59cc22f7880aca7654529a5d023fd6593c203" http://47.92.201.211:99/20260423-上海合合doris-202610121101.tar.gz</pre>
          </div>
          <p className="tips-hint">将以上命令完整复制后，粘贴到下载链接输入框即可自动识别。</p>
        </div>
      )}

      <div className="form-section">
        <div className="form-row">
          <label>下载链接</label>
         <input
           type="text"
           placeholder="http://47.92.201.211:99/xxx.tar.gz"
           value={url}
           onChange={(e) => setUrl(e.target.value)}
           onPaste={handleUrlPaste}
         />
        </div>
        <div className="form-row">
          <label>Token</label>
          <input
            type="password"
            placeholder="download_token"
            value={token}
            onChange={(e) => setToken(e.target.value)}
          />
        </div>
        <div className="save-row">
          <label style={{ width: 64, flexShrink: 0, color: "var(--text-dim)", fontSize: 13 }}>
            保存到
          </label>
          <input
            type="text"
            placeholder="下载目录"
            value={saveDir}
            onChange={(e) => setSaveDir(e.target.value)}
          />
          <button className="browse-btn" onClick={handleBrowse}>
            浏览
          </button>
        </div>
        <button
          className="start-btn"
          disabled={starting || !url.trim() || !token.trim() || !saveDir.trim()}
          onClick={handleStart}
        >
          {starting ? "创建中..." : "开始下载"}
        </button>
      </div>

      <div className="list-section">
        {downloads.length === 0 ? (
          <div className="empty-hint">暂无下载任务</div>
        ) : (
          downloads.map((d) => (
            <DownloadItem
              key={d.id}
              info={d}
              speed={speedMap[d.id] || 0}
              onPause={handlePause}
              onResume={handleResume}
           onCancel={handleCancel}
             onRemove={handleRemove}
             onOpenDir={handleOpenDir}
             onRedownload={handleRedownload}
            formatBytes={formatBytes}
              formatSpeed={formatSpeed}
            />
          ))
        )}
      </div>
    </div>
  );
}
