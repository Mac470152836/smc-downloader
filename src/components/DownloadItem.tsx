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

interface Props {
  info: DownloadInfo;
  speed: number;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onCancel: (id: string) => void;
  onRemove: (id: string) => void;
  formatBytes: (bytes: number) => string;
  formatSpeed: (bytesPerSec: number) => string;
}

export default function DownloadItem({
  info,
  speed,
  onPause,
  onResume,
  onCancel,
  onRemove,
  formatBytes,
  formatSpeed,
}: Props) {
  const percent =
    info.total_size > 0
      ? Math.min(100, (info.downloaded / info.total_size) * 100)
      : 0;

  const statusLabel: Record<string, string> = {
    downloading: "下载中",
    paused: "已暂停",
    completed: "已完成",
    error: "错误",
  };

  return (
    <div className="download-item">
      <div className="item-header">
        <span className="item-name" title={info.filename}>
          {info.filename}
        </span>
        <div className="item-actions">
          {(info.status === "downloading") && (
            <button
              className="icon-btn"
              onClick={() => onPause(info.id)}
              title="暂停"
            >
              ⏸
            </button>
          )}
          {(info.status === "paused" || info.status === "error") && (
            <button
              className="icon-btn"
              onClick={() => onResume(info.id)}
              title="继续"
            >
              ▶
            </button>
          )}
          {info.status !== "completed" && (
            <button
              className="icon-btn danger"
              onClick={() => onCancel(info.id)}
              title="取消并删除"
            >
              ✕
            </button>
          )}
          {info.status === "completed" && (
            <button
              className="icon-btn"
              onClick={() => onRemove(info.id)}
              title="从列表移除"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      <div className="progress-track">
        <div
          className={`progress-fill ${info.status}`}
          style={{ width: `${percent}%` }}
        />
      </div>

      <div className="item-meta">
        <span>
          {formatBytes(info.downloaded)}
          {info.total_size > 0 && ` / ${formatBytes(info.total_size)}`}
          {info.status === "downloading" && ` · ${formatSpeed(speed)}`}
        </span>
        <span className={`status-badge ${info.status}`}>
          {statusLabel[info.status] || info.status}
          {info.status === "downloading" && ` ${percent.toFixed(1)}%`}
          {info.status === "error" && info.error ? ` · ${info.error}` : ""}
        </span>
      </div>
    </div>
  );
}
