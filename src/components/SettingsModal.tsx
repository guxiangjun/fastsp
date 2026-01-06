import { useEffect, useState } from "react";
import { FolderOpen, Check, Loader2, Mic, X, Monitor, Keyboard, Languages, Sparkles, ChevronDown, ChevronUp, Globe, AlertCircle, Info, Upload } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, AppConfig, ModelVersion, ModelVersionsStatus, AudioDevice, LlmConfig, ProxyConfig, events } from "../lib/api";

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    isFirstSetup?: boolean;
}

export function SettingsModal({ isOpen, onClose, isFirstSetup = false }: SettingsModalProps) {
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

    // LLM state
    const [llmTesting, setLlmTesting] = useState(false);
    const [llmTestResult, setLlmTestResult] = useState<{ success: boolean; message: string } | null>(null);
    const [showPromptEditor, setShowPromptEditor] = useState(false);
    const [defaultPrompt, setDefaultPrompt] = useState("");

    // Download error state
    const [downloadError, setDownloadError] = useState<string | null>(null);

    // Import state
    const [importing, setImporting] = useState(false);

    // Close warning state
    const [showCloseWarning, setShowCloseWarning] = useState(false);

    useEffect(() => {
        if (isOpen) {
            api.getConfig().then(setConfig);
            api.getModelVersionsStatus().then(setVersionsStatus);
            api.getInputDevices().then(setInputDevices);
            api.getCurrentInputDevice().then(setCurrentDevice);
            api.getDefaultLlmPrompt().then(setDefaultPrompt);
            // Reset LLM test result when opening
            setLlmTestResult(null);
        }
    }, [isOpen]);

    useEffect(() => {
        // Only set up download event listeners when modal is open
        if (!isOpen) return;

        let isMounted = true;
        const unsubPromises = [
            events.onDownloadProgress((p) => {
                if (isMounted) setDownloadProgress(p);
            }),
            events.onDownloadComplete(() => {
                if (isMounted) {
                    setDownloading(null);
                    setDownloadProgress(null);
                    setDownloadError(null);
                    api.getModelVersionsStatus().then(status => {
                        if (isMounted) setVersionsStatus(status);
                    });
                }
            }),
            events.onDownloadError((error) => {
                if (isMounted) {
                    setDownloading(null);
                    setDownloadProgress(null);
                    setDownloadError(error || "Download failed. Please check your network and try again.");
                }
            }),
            events.onDownloadCancelled(() => {
                if (isMounted) {
                    setDownloading(null);
                    setDownloadProgress(null);
                    // No error message for user-initiated cancel
                }
            }),
            events.onImportStarted(() => {
                if (isMounted) setImporting(true);
            }),
            events.onImportComplete(() => {
                if (isMounted) {
                    setImporting(false);
                    setDownloadError(null);
                    api.getModelVersionsStatus().then(status => {
                        if (isMounted) setVersionsStatus(status);
                    });
                }
            }),
            events.onImportError((error) => {
                if (isMounted) {
                    setImporting(false);
                    setDownloadError(error || "Import failed. Please check the archive file.");
                }
            })
        ];

        return () => {
            isMounted = false;
            // Properly clean up all listeners
            unsubPromises.forEach(p => p.then(unsub => unsub()));
        };
    }, [isOpen]);

    const updateConfig = (key: keyof AppConfig, value: any) => {
        if (!config) return;
        const newConfig = { ...config, [key]: value };
        setConfig(newConfig);
        api.saveConfig(newConfig);
    };

    const updateLlmConfig = (key: keyof LlmConfig, value: any) => {
        if (!config) return;
        const newLlmConfig = { ...config.llm_config, [key]: value };
        const newConfig = { ...config, llm_config: newLlmConfig };
        setConfig(newConfig);
        api.saveConfig(newConfig);
        // Clear test result when config changes
        setLlmTestResult(null);
    };

    const updateProxyConfig = (key: keyof ProxyConfig, value: any) => {
        if (!config) return;
        const newProxyConfig = { ...config.proxy, [key]: value };
        const newConfig = { ...config, proxy: newProxyConfig };
        setConfig(newConfig);
        api.saveConfig(newConfig);
    };

    const handleTestLlm = async () => {
        if (!config) return;
        setLlmTesting(true);
        setLlmTestResult(null);
        try {
            const result = await api.testLlmConnection(config.llm_config, config.proxy);
            setLlmTestResult({ success: true, message: result });
        } catch (e: any) {
            setLlmTestResult({ success: false, message: e.toString() });
        } finally {
            setLlmTesting(false);
        }
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

    // Check if any operation is in progress (download or import)
    const isOperationInProgress = downloading !== null || importing;

    const handleModelAction = async (version: ModelVersion) => {
        if (!versionsStatus) return;
        // Block if any operation is in progress
        if (isOperationInProgress) return;

        const isDatadownloaded = version === "quantized" ? versionsStatus.quantized : versionsStatus.unquantized;

        if (!isDatadownloaded) {
            setDownloading(version);
            setDownloadError(null); // Clear previous errors
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

    const handleImportModel = async () => {
        // Block if any operation is in progress
        if (isOperationInProgress) return;

        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: "Model Archive", extensions: ["bz2", "tar.bz2"] }],
            });

            if (selected && typeof selected === "string") {
                setDownloadError(null);
                // Import as quantized version by default
                await api.importModel(selected, "quantized");
            }
        } catch (e) {
            console.error("Failed to open file dialog:", e);
        }
    };

    // Check if setup requirements are met
    // Use config.input_device for checking if user has ever selected a device
    // Use currentDevice for display purposes
    const isDeviceSelected = (config?.input_device && config.input_device !== "") || (currentDevice && currentDevice !== "");
    const isModelDownloaded = versionsStatus?.quantized || versionsStatus?.unquantized;

    // Handle close with validation for first setup
    const handleClose = () => {
        // If downloading, show warning about canceling download
        if (isOperationInProgress) {
            setShowCloseWarning(true);
            return;
        }
        if (isFirstSetup && (!isDeviceSelected || !isModelDownloaded)) {
            setShowCloseWarning(true);
            return;
        }
        onClose();
    };

    // Force close (user confirmed) - also cancel any ongoing operations
    const handleForceClose = async () => {
        setShowCloseWarning(false);
        if (downloading) {
            await api.cancelDownload();
        }
        onClose();
    };

    if (!isOpen || !config) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/20 backdrop-blur-sm animate-in fade-in duration-200" onClick={handleClose}>
            <div className="bg-white border border-slate-200 w-full max-w-2xl max-h-[85vh] rounded-2xl shadow-2xl flex flex-col overflow-hidden animate-in zoom-in-95 duration-200" onClick={(e) => e.stopPropagation()}>
                <div className="flex justify-between items-center p-6 border-b border-slate-100 bg-slate-50/50">
                    <h2 className="text-xl font-bold text-slate-800">{isFirstSetup ? "Welcome! Let's Get Started" : "Settings"}</h2>
                    <button onClick={handleClose} className="p-2 hover:bg-slate-200/50 rounded-full transition-colors text-slate-400 hover:text-slate-600">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto p-6 space-y-8 custom-scrollbar">
                    {/* First Setup Guide Banner */}
                    {isFirstSetup && (
                        <div className="bg-gradient-to-r from-chinese-indigo/10 to-chinese-indigo/5 border border-chinese-indigo/20 rounded-xl p-4">
                            <div className="flex items-start gap-3">
                                <Info className="w-5 h-5 text-chinese-indigo flex-shrink-0 mt-0.5" />
                                <div>
                                    <h3 className="font-semibold text-slate-800 mb-1">Setup Required</h3>
                                    <p className="text-sm text-slate-600">
                                        Please complete these steps before using the app:
                                    </p>
                                    <ul className="mt-2 space-y-1.5 text-sm">
                                        <li className={`flex items-center gap-2 ${isDeviceSelected ? "text-green-600" : "text-slate-600"}`}>
                                            {isDeviceSelected ? <Check className="w-4 h-4" /> : <span className="w-4 h-4 rounded-full border-2 border-current flex-shrink-0" />}
                                            Select an input device (microphone)
                                        </li>
                                        <li className={`flex items-center gap-2 ${isModelDownloaded ? "text-green-600" : "text-slate-600"}`}>
                                            {isModelDownloaded ? <Check className="w-4 h-4" /> : <span className="w-4 h-4 rounded-full border-2 border-current flex-shrink-0" />}
                                            Download a speech recognition model
                                        </li>
                                    </ul>
                                </div>
                            </div>
                        </div>
                    )}

                    {/* Download Error Alert */}
                    {downloadError && (
                        <div className="bg-red-50 border border-red-200 rounded-xl p-4 animate-in slide-in-from-top-2 duration-200">
                            <div className="flex items-start gap-3">
                                <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
                                <div className="flex-1">
                                    <h3 className="font-semibold text-red-800 mb-1">Download Failed</h3>
                                    <p className="text-sm text-red-600">{downloadError}</p>
                                    <button
                                        onClick={() => setDownloadError(null)}
                                        className="mt-2 text-xs text-red-500 hover:text-red-700 underline"
                                    >
                                        Dismiss
                                    </button>
                                </div>
                            </div>
                        </div>
                    )}

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
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                            <ModelCard
                                title="Quantized (Fast)"
                                size="~230MB"
                                active={versionsStatus?.current === "quantized"}
                                downloaded={versionsStatus?.quantized}
                                downloading={downloading === "quantized"}
                                switching={switchingModel && versionsStatus?.current !== "quantized"}
                                progress={downloadProgress}
                                onClick={() => handleModelAction("quantized")}
                                onCancel={() => api.cancelDownload()}
                                disabled={isOperationInProgress && downloading !== "quantized"}
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
                                onCancel={() => api.cancelDownload()}
                                disabled={isOperationInProgress && downloading !== "unquantized"}
                            />
                            <ImportCard
                                importing={importing}
                                disabled={isOperationInProgress && !importing}
                                onClick={handleImportModel}
                            />
                        </div>
                        <div className="mt-4 flex justify-end">
                            <button onClick={api.openModelFolder} className="text-xs text-slate-400 hover:text-chinese-indigo flex items-center gap-1 transition-colors">
                                <FolderOpen className="w-3 h-3" /> Open Model Folder
                            </button>
                        </div>
                    </section>

                    {/* LLM Correction */}
                    <section>
                        <SectionHeader icon={Sparkles} title="LLM Correction" />
                        <div className="space-y-4">
                            {/* Enable Toggle */}
                            <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3">
                                        <Sparkles className="w-5 h-5 text-slate-400" />
                                        <div>
                                            <div className="font-medium text-slate-800">Enable LLM Correction</div>
                                            <div className="text-xs text-slate-500">Use AI to fix transcription errors</div>
                                        </div>
                                    </div>
                                    <button
                                        onClick={() => updateLlmConfig("enabled", !config?.llm_config.enabled)}
                                        className={`relative w-12 h-6 rounded-full transition-colors ${config?.llm_config.enabled ? "bg-chinese-indigo" : "bg-slate-300"}`}
                                    >
                                        <div className={`absolute top-1 w-4 h-4 rounded-full bg-white shadow transition-all ${config?.llm_config.enabled ? "left-7" : "left-1"}`} />
                                    </button>
                                </div>
                            </div>

                            {/* LLM Config Fields - Only show when enabled */}
                            {config?.llm_config.enabled && (
                                <div className="space-y-3">
                                    {/* Base URL */}
                                    <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                        <label className="block text-sm font-medium text-slate-700 mb-2">Base URL</label>
                                        <input
                                            type="text"
                                            value={config.llm_config.base_url}
                                            onChange={(e) => updateLlmConfig("base_url", e.target.value)}
                                            placeholder="https://api.openai.com/v1"
                                            className="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none"
                                        />
                                    </div>

                                    {/* API Key */}
                                    <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                        <label className="block text-sm font-medium text-slate-700 mb-2">API Key</label>
                                        <input
                                            type="password"
                                            value={config.llm_config.api_key}
                                            onChange={(e) => updateLlmConfig("api_key", e.target.value)}
                                            placeholder="sk-..."
                                            className="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none"
                                        />
                                    </div>

                                    {/* Model */}
                                    <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                        <label className="block text-sm font-medium text-slate-700 mb-2">Model</label>
                                        <input
                                            type="text"
                                            value={config.llm_config.model}
                                            onChange={(e) => updateLlmConfig("model", e.target.value)}
                                            placeholder="gpt-4o-mini"
                                            className="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none"
                                        />
                                    </div>

                                    {/* Custom Prompt (Collapsible) */}
                                    <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                        <button
                                            onClick={() => setShowPromptEditor(!showPromptEditor)}
                                            className="w-full flex items-center justify-between text-sm font-medium text-slate-700"
                                        >
                                            <span>Custom Prompt</span>
                                            {showPromptEditor ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                                        </button>
                                        {showPromptEditor && (
                                            <div className="mt-3 space-y-2">
                                                <textarea
                                                    value={config.llm_config.custom_prompt || ""}
                                                    onChange={(e) => updateLlmConfig("custom_prompt", e.target.value)}
                                                    placeholder={defaultPrompt}
                                                    rows={8}
                                                    className="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none resize-none font-mono"
                                                />
                                                <p className="text-xs text-slate-400">
                                                    Use {"{"}<span>text</span>{"}"} as placeholder for the transcribed text. Leave empty to use default prompt.
                                                </p>
                                            </div>
                                        )}
                                    </div>

                                    {/* Test Connection Button */}
                                    <div className="flex items-center gap-3">
                                        <button
                                            onClick={handleTestLlm}
                                            disabled={llmTesting || !config.llm_config.api_key}
                                            className="px-4 py-2 bg-chinese-indigo text-white rounded-lg text-sm font-medium hover:bg-chinese-indigo/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                                        >
                                            {llmTesting && <Loader2 className="w-4 h-4 animate-spin" />}
                                            {llmTesting ? "Testing..." : "Test Connection"}
                                        </button>
                                        {llmTestResult && (
                                            <span className={`text-sm ${llmTestResult.success ? "text-green-600" : "text-red-600"}`}>
                                                {llmTestResult.success ? "Success!" : llmTestResult.message.slice(0, 50)}
                                            </span>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>
                    </section>

                    {/* Network / Proxy */}
                    <section>
                        <SectionHeader icon={Globe} title="Network" />
                        <div className="space-y-4">
                            {/* Enable Proxy Toggle */}
                            <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3">
                                        <Globe className="w-5 h-5 text-slate-400" />
                                        <div>
                                            <div className="font-medium text-slate-800">Enable Proxy</div>
                                            <div className="text-xs text-slate-500">Use proxy for network requests</div>
                                        </div>
                                    </div>
                                    <button
                                        onClick={() => updateProxyConfig("enabled", !config?.proxy.enabled)}
                                        className={`relative w-12 h-6 rounded-full transition-colors ${config?.proxy.enabled ? "bg-chinese-indigo" : "bg-slate-300"}`}
                                    >
                                        <div className={`absolute top-1 w-4 h-4 rounded-full bg-white shadow transition-all ${config?.proxy.enabled ? "left-7" : "left-1"}`} />
                                    </button>
                                </div>
                            </div>

                            {/* Proxy URL - Only show when enabled */}
                            {config?.proxy.enabled && (
                                <div className="bg-slate-50 p-4 rounded-xl border border-slate-200">
                                    <label className="block text-sm font-medium text-slate-700 mb-2">Proxy URL</label>
                                    <input
                                        type="text"
                                        value={config.proxy.url}
                                        onChange={(e) => updateProxyConfig("url", e.target.value)}
                                        placeholder="http://127.0.0.1:7890 or socks5://127.0.0.1:1080"
                                        className="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 focus:ring-2 focus:ring-chinese-indigo outline-none"
                                    />
                                    <p className="text-xs text-slate-400 mt-2">
                                        Supports HTTP and SOCKS5 proxies. Used for model downloads and LLM API requests.
                                    </p>
                                </div>
                            )}
                        </div>
                    </section>
                </div>
            </div>

            {/* Close Warning Modal - positioned outside the main modal */}
            {showCloseWarning && (
                <div className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/30 backdrop-blur-sm animate-in fade-in duration-150" onClick={(e) => { e.stopPropagation(); setShowCloseWarning(false); }}>
                    <div className="bg-white rounded-xl shadow-2xl max-w-sm w-full p-6 animate-in zoom-in-95 duration-150" onClick={e => e.stopPropagation()}>
                        <div className="flex items-center gap-3 mb-4">
                            <div className="w-10 h-10 rounded-full bg-amber-100 flex items-center justify-center">
                                <AlertCircle className="w-5 h-5 text-amber-600" />
                            </div>
                            <h3 className="font-semibold text-slate-800">
                                {isOperationInProgress ? "Operation in Progress" : "Setup Incomplete"}
                            </h3>
                        </div>
                        <p className="text-sm text-slate-600 mb-4">
                            {isOperationInProgress
                                ? (downloading ? "A model is being downloaded. Closing will cancel the download." : "A model is being imported. Please wait for it to complete.")
                                : !isDeviceSelected && !isModelDownloaded
                                    ? "Please select an input device and download a model to use the app."
                                    : !isDeviceSelected
                                        ? "Please select an input device to use the app."
                                        : "Please download a model to use the app."
                            }
                        </p>
                        <div className="flex gap-3 justify-end">
                            {/* Only show close button if not importing (can cancel download but not import) */}
                            {!importing && (
                                <button
                                    onClick={(e) => { e.stopPropagation(); handleForceClose(); }}
                                    className="px-4 py-2 text-sm text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
                                >
                                    {downloading ? "Cancel & Close" : "Close Anyway"}
                                </button>
                            )}
                            <button
                                onClick={(e) => { e.stopPropagation(); setShowCloseWarning(false); }}
                                className="px-4 py-2 text-sm bg-chinese-indigo text-white rounded-lg hover:bg-chinese-indigo/90 transition-colors"
                            >
                                {isOperationInProgress ? "Continue" : "Continue Setup"}
                            </button>
                        </div>
                    </div>
                </div>
            )}
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

function ModelCard({ title, size, active, downloaded, downloading, switching, progress, onClick, onCancel, disabled }: any) {
    const progressPercent = progress && progress.total > 0 ? Math.round((progress.current / progress.total) * 100) : 0;

    return (
        <button
            onClick={onClick}
            disabled={downloading || switching || disabled}
            className={`relative p-4 rounded-xl text-left transition-all duration-200 overflow-hidden outline-none focus:ring-2 focus:ring-chinese-indigo/50 focus:ring-offset-1 ${active
                ? "bg-chinese-indigo/50 text-white shadow-lg shadow-chinese-indigo/25"
                : "bg-white hover:bg-slate-200"
                } ${disabled && !downloading ? "opacity-50 cursor-not-allowed" : ""}`}
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
                            <div className="w-full flex justify-between items-center text-xs text-chinese-indigo mb-1">
                                <span>Downloading...</span>
                                <div className="flex items-center gap-2">
                                    <span>{progressPercent}%</span>
                                    <button
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            onCancel?.();
                                        }}
                                        className="p-1 hover:bg-slate-200 rounded-full transition-colors"
                                        title="Cancel download"
                                    >
                                        <X className="w-3.5 h-3.5 text-slate-500 hover:text-red-500" />
                                    </button>
                                </div>
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

function ImportCard({ importing, disabled, onClick }: { importing: boolean; disabled: boolean; onClick: () => void }) {
    return (
        <button
            onClick={onClick}
            disabled={importing || disabled}
            className={`relative p-4 rounded-xl text-left transition-all duration-200 overflow-hidden outline-none focus:ring-2 focus:ring-chinese-indigo/50 focus:ring-offset-1 bg-white hover:bg-slate-200 border-2 border-dashed border-slate-300 hover:border-chinese-indigo/50 ${disabled && !importing ? "opacity-50 cursor-not-allowed" : ""}`}
        >
            <div className="flex justify-between items-start mb-2">
                <span className="text-xs font-bold px-2 py-0.5 rounded-full bg-slate-100 text-slate-400">
                    Manual Import
                </span>
            </div>

            <div className="font-semibold mb-0.5 text-slate-800">Import Archive</div>
            <div className="text-xs text-slate-400">.tar.bz2 file</div>

            <div className="mt-3 flex justify-center">
                <Upload className="w-6 h-6 text-slate-400" />
            </div>

            {importing && (
                <div className="absolute inset-0 bg-white/90 backdrop-blur-sm flex flex-col items-center justify-center p-4 rounded-xl">
                    <Loader2 className="w-5 h-5 animate-spin text-chinese-indigo mb-2" />
                    <span className="text-xs text-chinese-indigo">Importing...</span>
                </div>
            )}
        </button>
    )
}
