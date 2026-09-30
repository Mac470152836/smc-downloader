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

export default function App() {
  const [url, setUrl] = useState("");
  const [token, setToken] = useState("");
  const [saveDir, setSaveDir] = useState("");
  const [downloads, setDownloads] = useState<DownloadInfo[]>([]);
  const [speedMap, setSpeedMap] = useState<Record<string, number>>({});
  const [starting, setStarting] = useState(false);

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

  return (
    <div className="app">
      <div className="header">
        <span className="logo">📦</span>
        <span>SMC 下载器</span>
      </div>

      <div className="form-section">
        <div className="form-row">
          <label>下载链接</label>
          <input
            type="text"
            placeholder="http://47.92.201.211:99/xxx.tar.gz"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
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
              formatBytes={formatBytes}
              formatSpeed={formatSpeed}
            />
          ))
        )}
      </div>
    </div>
  );
}
