import { EXPORT_PRESETS, applyExportPreset } from "../../types";
import { useProjectStore } from "../../state/projectStore";

export function ExportPanel() {
  const { exportSettings, setExportSettings } = useProjectStore();

  return (
    <div className="export-panel">
      <label className="export-preset-label">
        Kvalitet
        <select
          value={exportSettings.preset}
          onChange={(e) => setExportSettings(applyExportPreset(exportSettings, e.target.value))}
        >
          {EXPORT_PRESETS.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
      </label>
      <span className="export-preset-details">
        {exportSettings.maxWidth}px bred · {exportSettings.audioBitrateKbps} kbps ljud
      </span>
    </div>
  );
}
