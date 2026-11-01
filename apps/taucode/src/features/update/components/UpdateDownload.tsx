import { useUpdateDownload } from "../hooks/useUpdateDownload";

export function UpdateDownload({ version, available }: { version: string; available: boolean }) {
  const download = useUpdateDownload(version);
  return <div className="update-download">
    {download.progress ? <span role="status">Downloading… <progress aria-label="Update download" max={download.progress.total || 1} value={download.progress.total ? download.progress.downloaded : undefined} /></span>
      : download.path ? <><span role="status">Saved to Downloads. Open the installer to finish updating.</span> <button className="primary" onClick={() => void download.open()}>Open installer</button></>
      : available ? <button className="primary" onClick={() => void download.start()}>Download update</button>
      : <span>No compatible installer is published for this computer.</span>}
    {download.error && <p role="alert">{download.error}</p>}
  </div>;
}
