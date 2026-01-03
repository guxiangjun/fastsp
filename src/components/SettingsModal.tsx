import { useEffect, useState } from "react";
import { FolderOpen, Check, Loader2, Mic, X, Monitor, Keyboard, Languages } from "lucide-react";
import { api, AppConfig, ModelVersion, ModelVersionsStatus, AudioDevice, events } from "../lib/api";

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
}

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
    const [config, setConfig] = useState<AppConfig | null>(null);
    const [versionsStatus, setVersionsStatus] = useState<ModelVersionsStatus | null>(null);
    const [inputDevices, setInputDevices] = useState<AudioDevice[]>([]);
    const [currentDevice, setCurrentDevice] = useState("");

    // Loading states
    const [downloading, setDownloading] = useState<ModelVersion | null>(null);
    const [downloadProgress, setDownloadProgress] = useState<{ current: number; total: number } | null>(null);
    const [switchingDevice, setSwitchingDevice] = useState(false);
    const [switchingModel, setSwitchingModel] = useState(false);

    // Audio test state
    const [isTesting, setIsTesting] = useState(false);
    const [audioLevel, setAudioLevel] = useState(0);

    useEffect(() => {
        if (isOpen) {
            api.getConfig().then(setConfig);
            api.getModelVersionsStatus().then(setVersionsStatus);
            api.getInputDevices().then(setInputDevices);
            api.getCurrentInputDevice().then(setCurrentDevice);
        }
    }, [isOpen]);

    useEffect(() => {
        const unsubs = [
            events.onDownloadProgress((p) => setDownloadProgress(p)),
            events.onDownloadComplete(() => {
                setDownloading(null);
                api.getModelVersionsStatus().then(setVersionsStatus);
            }),
            events.onDownloadError(() => setDownloading(null))
        ];
        return () => { unsubs.forEach(u => u.then(f => f())); };
    }, []);

    const updateConfig = (key: keyof AppConfig, value: any) => {
        if (!config) return;
        const newConfig = { ...config, [key]: value };
        setConfig(newConfig);
        api.saveConfig(newConfig);
    };

    const handleSwitchDevice = async (deviceName: string) => {
        // Stop test if running when switching devices
        if (isTesting) {
            await api.stopAudioTest();
            setIsTesting(false);
            setAudioLevel(0);
        }
        setSwitchingDevice(true);
        try {
            await api.switchInputDevice(deviceName);
            setCurrentDevice(deviceName);
            if (config) updateConfig("input_device", deviceName);
        } finally {
            setSwitchingDevice(false);
        }
    };

    const toggleAudioTest = async () => {
        if (isTesting) {
            await api.stopAudioTest();
            setIsTesting(false);
            setAudioLevel(0);
        } else {
            await api.startAudioTest();
            setIsTesting(true);
        }
    };

    // Audio level listener
    useEffect(() => {
        if (!isTesting) return;
        const unsub = events.onAudioLevel((level) => {
            // Clamp and scale level for visual display (0-1 range)
            setAudioLevel(Math.min(1, level * 5));
        });
        return () => { unsub.then(f => f()); };
    }, [isTesting]);

    // Stop test when modal closes
    useEffect(() => {
        if (!isOpen && isTesting) {
            api.stopAudioTest();
            setIsTesting(false);
            setAudioLevel(0);
        }
    }, [isOpen, isTesting]);

    const handleModelAction = async (version: ModelVersion) => {
        if (!versionsStatus) return;
        const isDatadownloaded = version === "quantized" ? versionsStatus.quantized : versionsStatus.unquantized;

        if (!isDatadownloaded) {
            setDownloading(version);
            await api.downloadModelForVersion(version);
        } else if (versionsStatus.current !== version) {
            setSwitchingModel(true);
            try {
                await api.switchModelVersion(version);
                setVersionsStatus(prev => prev ? ({ ...prev, current: version }) : null);
                if (config) updateConfig("model_version", version);
            } finally {
                setSwitchingModel(false);
            }
        }
    };

    if (!isOpen || !config) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/20 backdrop-blur-sm animate-in fade-in duration-200" onClick={onClose}>
            <div className="bg-white border border-slate-200 w-full max-w-2xl max-h-[85vh] rounded-2xl shadow-2xl flex flex-col overflow-hidden animate-in zoom-in-95 duration-200" onClick={(e) => e.stopPropagation()}>
                <div className="flex justify-between items-center p-6 border-b border-slate-100 bg-slate-50/50">
                    <h2 className="text-xl font-bold text-slate-800">Settings</h2>
                    <button onClick={onClose} className="p-2 hover:bg-slate-200/50 rounded-full transition-colors text-slate-400 hover:text-slate-600">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto p-6 space-y-8 custom-scrollbar">
                    {/* Triggers Section */}
                    <section>
                        <SectionHeader icon={Keyboard} title="Triggers" />
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                            <TriggerToggle
                                label="Mouse Mode"
                                desc="Middle Click Hold"
                                active={config.trigger_mouse}
                                onClick={() => updateConfig("trigger_mouse", !config.trigger_mouse)}
                            />
                            <TriggerToggle
                                label="Hold Mode"
                                desc="Ctrl + Win Hold"
                                active={config.trigger_hold}
                                onClick={() => updateConfig("trigger_hold", !config.trigger_hold)}
                            />
                            <TriggerToggle
                                label="Toggle Mode"
                                desc="Right Alt Press"
                                active={config.trigger_toggle}
                                onClick={() => updateConfig("trigger_toggle", !config.trigger_toggle)}
                            />
                        </div>
                    </section>

                    {/* Audio & Language */}
                    <section>
                        <SectionHeader icon={Mic} title="Audio & Language" />
                        <div className="space-y-4">
                            <div className="bg-slate-50 p-4 rounded-xl border border-slate-200 flex flex-col md:flex-row gap-4 items-center justify-between">
                                <div className="flex items-center gap-3 w-full md:w-auto">
                                    <Languages className="w-5 h-5 text-slate-400" />
                                    <div>
                                        <div className="font-medium text-slate-800">Transcription Language</div>
                                        <div className="text-xs text-slate-500">Target language</div>
                                    </div>
                                </div>
                                <select
                                    className="w-full md:w-48 bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none"
                                    value={config.language}
                                    onChange={(e) => updateConfig("language", e.target.value)}
                                >
                                    <option value="">Auto Detect</option>
                                    <option value="zh">Chinese</option>
                                    <option value="en">English</option>
                                    <option value="ja">Japanese</option>
                                    <option value="ko">Korean</option>
                                    <option value="yue">Cantonese</option>
                                </select>
                            </div>

                            <div className="bg-slate-50 p-4 rounded-xl border border-slate-200 space-y-3">
                                <div className="flex flex-col md:flex-row gap-4 items-center justify-between">
                                    <div className="flex items-center gap-3 w-full md:w-auto">
                                        <Mic className="w-5 h-5 text-slate-400" />
                                        <div>
                                            <div className="font-medium text-slate-800">Input Device</div>
                                            <div className="text-xs text-slate-500">Current microphone</div>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-2 w-full md:w-auto">
                                        {switchingDevice && <Loader2 className="w-4 h-4 animate-spin text-chinese-indigo" />}
                                        <select
                                            className="flex-1 md:w-48 bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none disabled:opacity-50"
                                            value={currentDevice}
                                            onChange={(e) => handleSwitchDevice(e.target.value)}
                                            disabled={switchingDevice || isTesting}
                                        >
                                            {inputDevices.map(d => (
                                                <option key={d.name} value={d.name}>{d.name} {d.is_default ? "(Default)" : ""}</option>
                                            ))}
                                        </select>
                                        <button
                                            onClick={toggleAudioTest}
                                            className={`px-3 py-2 rounded-lg text-sm font-medium transition-all ${isTesting
                                                ? "bg-chinese-indigo text-white"
                                                : "bg-white border border-slate-200 text-slate-600 hover:border-chinese-indigo hover:text-chinese-indigo"
                                                }`}
                                            disabled={switchingDevice}
                                        >
                                            {isTesting ? "Stop" : "Test"}
                                        </button>
                                    </div>
                                </div>
                                {/* Volume Bar */}
                                {isTesting && (
                                    <div className="flex items-center gap-3">
                                        <div className="text-xs text-slate-500 w-12">Level:</div>
                                        <div className="flex-1 h-2 bg-slate-200 rounded-full overflow-hidden">
                                            <div
                                                className="h-full bg-gradient-to-r from-green-400 via-yellow-400 to-red-500 transition-all duration-75"
                                                style={{ width: `${audioLevel * 100}%` }}
                                            />
                                        </div>
                                    </div>
                                )}
                            </div>
                        </div>
                    </section>

                    {/* Model Versions */}
                    <section>
                        <SectionHeader icon={Monitor} title="Engine" />
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <ModelCard
                                title="Quantized (Fast)"
                                size="~230MB"
                                active={versionsStatus?.current === "quantized"}
                                downloaded={versionsStatus?.quantized}
                                downloading={downloading === "quantized"}
                                switching={switchingModel && versionsStatus?.current !== "quantized"}
                                progress={downloadProgress}
                                onClick={() => handleModelAction("quantized")}
                            />
                            <ModelCard
                                title="Unquantized (Precise)"
                                size="~820MB"
                                active={versionsStatus?.current === "unquantized"}
                                downloaded={versionsStatus?.unquantized}
                                downloading={downloading === "unquantized"}
                                switching={switchingModel && versionsStatus?.current !== "unquantized"}
                                progress={downloadProgress}
                                onClick={() => handleModelAction("unquantized")}
                            />
                        </div>
                        <div className="mt-4 flex justify-end">
                            <button onClick={api.openModelFolder} className="text-xs text-slate-400 hover:text-chinese-indigo flex items-center gap-1 transition-colors">
                                <FolderOpen className="w-3 h-3" /> Open Model Folder
                            </button>
                        </div>
                    </section>
                </div>
            </div>
        </div>
    );
}

function SectionHeader({ icon: Icon, title }: any) {
    return (
        <div className="flex items-center gap-2 mb-4 text-chinese-indigo">
            <Icon className="w-4 h-4" />
            <h3 className="text-sm font-bold uppercase tracking-wider">{title}</h3>
        </div>
    )
}

function TriggerToggle({ label, desc, active, onClick }: any) {
    return (
        <button
            onClick={onClick}
            className={`p-4 rounded-xl text-left transition-all duration-200 ${active
                ? "bg-chinese-indigo/50 text-white shadow-lg shadow-chinese-indigo/25"
                : "bg-white hover:bg-slate-200"
                }`}
        >
            <div className={`w-full flex justify-between items-center mb-2`}>
                <div className={`w-2 h-2 rounded-full ${active ? "bg-white shadow-[0_0_8px_rgba(255,255,255,0.6)]" : "bg-slate-300"}`} />
                {active && <Check className="w-3 h-3 text-white" />}
            </div>
            <div className={`font-semibold ${active ? "text-white" : "text-slate-800"}`}>{label}</div>
            <div className={`text-xs mt-1 ${active ? "text-white/80" : "text-slate-400"}`}>{desc}</div>
        </button>
    )
}

function ModelCard({ title, size, active, downloaded, downloading, switching, progress, onClick }: any) {
    const progressPercent = progress && progress.total > 0 ? Math.round((progress.current / progress.total) * 100) : 0;

    return (
        <button
            onClick={onClick}
            disabled={downloading || switching}
            className={`relative p-4 rounded-xl text-left transition-all duration-200 overflow-hidden outline-none focus:ring-2 focus:ring-chinese-indigo/50 focus:ring-offset-1 ${active
                ? "bg-chinese-indigo/50 text-white shadow-lg shadow-chinese-indigo/25"
                : "bg-white hover:bg-slate-200"
                }`}
        >
            <div className="flex justify-between items-start mb-2">
                <span className={`text-xs font-bold px-2 py-0.5 rounded-full ${active ? "bg-white/20 text-white" : "bg-slate-100 text-slate-400"
                    }`}>
                    {active ? "Active" : downloaded ? "Ready" : "Not Downloaded"}
                </span>
                {active && <Check className="w-4 h-4 text-white" />}
            </div>

            <div className={`font-semibold mb-0.5 ${active ? "text-white" : "text-slate-800"}`}>{title}</div>
            <div className={`text-xs ${active ? "text-white/70" : "text-slate-400"}`}>{size}</div>

            {(downloading || switching) && (
                <div className="absolute inset-0 bg-white/90 backdrop-blur-sm flex flex-col items-center justify-center p-4 rounded-xl">
                    {downloading ? (
                        <>
                            <div className="w-full flex justify-between text-xs text-chinese-indigo mb-1">
                                <span>Downloading...</span>
                                <span>{progressPercent}%</span>
                            </div>
                            <div className="w-full h-1 bg-slate-200 rounded-full overflow-hidden">
                                <div className="h-full bg-chinese-indigo transition-all duration-300" style={{ width: `${progressPercent}%` }} />
                            </div>
                        </>
                    ) : (
                        <div className="flex items-center gap-2 text-chinese-indigo text-sm">
                            <Loader2 className="w-4 h-4 animate-spin" /> Switching Engine...
                        </div>
                    )}
                </div>
            )}
        </button>
    )
}
