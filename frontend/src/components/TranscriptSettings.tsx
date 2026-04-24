import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { Eye, EyeOff, Lock, Unlock } from 'lucide-react';
import { ModelManager } from './WhisperModelManager';
import { ParakeetModelManager } from './ParakeetModelManager';
import { QwenAsrModelManager } from './QwenAsrModelManager';


export interface TranscriptModelProps {
    provider: 'localWhisper' | 'parakeet' | 'qwenAsr' | 'deepgram' | 'elevenLabs' | 'groq' | 'openai';
    model: string;
    apiKey?: string | null;
}

export interface TranscriptSettingsProps {
    transcriptModelConfig: TranscriptModelProps;
    setTranscriptModelConfig: (config: TranscriptModelProps) => void;
    onModelSelect?: () => void;
}

type LocalProvider = 'localWhisper' | 'parakeet' | 'qwenAsr';
type CloudProvider = Exclude<TranscriptModelProps['provider'], LocalProvider>;

const MODEL_OPTIONS: Record<TranscriptModelProps['provider'], string[]> = {
    localWhisper: [],
    parakeet: [],
    qwenAsr: [],
    deepgram: ['nova-2-phonecall'],
    elevenLabs: ['eleven_multilingual_v2'],
    groq: ['llama-3.3-70b-versatile'],
    openai: ['gpt-4o-mini-transcribe', 'gpt-4o-transcribe', 'whisper-1'],
};

function isLocalProvider(provider: TranscriptModelProps['provider']): provider is LocalProvider {
    return provider === 'localWhisper' || provider === 'parakeet' || provider === 'qwenAsr';
}

export function TranscriptSettings({ transcriptModelConfig, setTranscriptModelConfig, onModelSelect }: TranscriptSettingsProps) {
    const [apiKey, setApiKey] = useState<string | null>(transcriptModelConfig.apiKey || null);
    const [showApiKey, setShowApiKey] = useState<boolean>(false);
    const [isApiKeyLocked, setIsApiKeyLocked] = useState<boolean>(true);
    const [isLockButtonVibrating, setIsLockButtonVibrating] = useState<boolean>(false);
    const [uiProvider, setUiProvider] = useState<TranscriptModelProps['provider']>(transcriptModelConfig.provider);

    // Sync uiProvider when backend config changes (e.g., after model selection or initial load)
    useEffect(() => {
        setUiProvider(transcriptModelConfig.provider);
    }, [transcriptModelConfig.provider]);

    useEffect(() => {
        if (isLocalProvider(uiProvider)) {
            setApiKey(null);
            return;
        }
        if (transcriptModelConfig.provider === uiProvider) {
            setApiKey(transcriptModelConfig.apiKey || null);
        }
    }, [transcriptModelConfig.provider, transcriptModelConfig.apiKey, uiProvider]);

    const fetchApiKey = async (provider: CloudProvider): Promise<string | null> => {
        try {
            const data = await invoke('api_get_transcript_api_key', { provider }) as string;
            const normalized = data?.trim() ? data.trim() : null;
            setApiKey(normalized);
            return normalized;
        } catch (err) {
            console.error('Error fetching API key:', err);
            setApiKey(null);
            return null;
        }
    };

    const persistCloudConfig = async (provider: CloudProvider, model: string, key: string | null) => {
        const normalizedKey = key?.trim() ? key.trim() : null;
        await invoke('api_save_transcript_config', {
            provider,
            model,
            apiKey: normalizedKey,
        });
        setTranscriptModelConfig({
            provider,
            model,
            apiKey: normalizedKey,
        });
    };

    const requiresApiKey = !isLocalProvider(uiProvider);
    const cloudModelOptions = isLocalProvider(uiProvider) ? [] : MODEL_OPTIONS[uiProvider];
    const selectedCloudModel = transcriptModelConfig.provider === uiProvider
        ? transcriptModelConfig.model
        : (cloudModelOptions[0] || '');

    const handleInputClick = () => {
        if (isApiKeyLocked) {
            setIsLockButtonVibrating(true);
            setTimeout(() => setIsLockButtonVibrating(false), 500);
        }
    };

    const handleWhisperModelSelect = (modelName: string) => {
        // Always update config when model is selected, regardless of current provider
        // This ensures the model is set when user switches back
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'localWhisper', // Ensure provider is set correctly
            model: modelName
        });
        // Close modal after selection
        if (onModelSelect) {
            onModelSelect();
        }
    };

    const handleParakeetModelSelect = (modelName: string) => {
        // Always update config when model is selected, regardless of current provider
        // This ensures the model is set when user switches back
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'parakeet', // Ensure provider is set correctly
            model: modelName
        });
        // Close modal after selection
        if (onModelSelect) {
            onModelSelect();
        }
    };

    const handleQwenAsrModelSelect = (modelName: string) => {
        // Always update config when model is selected, regardless of current provider
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'qwenAsr',
            model: modelName
        });
        if (onModelSelect) {
            onModelSelect();
        }
    };

    return (
        <div>
            <div>
                {/* <div className="flex justify-between items-center mb-4">
                    <h3 className="text-lg font-semibold text-gray-900">Transcript Settings</h3>
                </div> */}
                <div className="space-y-4 pb-6">
                    <div>
                        <Label className="block text-sm font-medium text-gray-700 mb-1">
                            Transcript Model
                        </Label>
                        <div className="flex space-x-2 mx-1">
                            <Select
                                value={uiProvider}
                                onValueChange={(value) => {
                                    const provider = value as TranscriptModelProps['provider'];
                                    setUiProvider(provider);

                                    if (isLocalProvider(provider)) {
                                        setTranscriptModelConfig({
                                            provider,
                                            model: transcriptModelConfig.provider === provider ? transcriptModelConfig.model : '',
                                            apiKey: null,
                                        });
                                        return;
                                    }

                                    const initialModel =
                                        transcriptModelConfig.provider === provider && transcriptModelConfig.model
                                            ? transcriptModelConfig.model
                                            : (MODEL_OPTIONS[provider][0] || '');

                                    void (async () => {
                                        try {
                                            const existingApiKey = await fetchApiKey(provider);
                                            await persistCloudConfig(provider, initialModel, existingApiKey);
                                        } catch (err) {
                                            console.error('Failed to persist transcript provider config:', err);
                                        }
                                    })();
                                }}
                            >
                                <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                    <SelectValue placeholder="Select provider" />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="parakeet">⚡ Parakeet (Recommended - Real-time / Accurate)</SelectItem>
                                    <SelectItem value="qwenAsr">🧠 Qwen3 ASR (Multilingual / Accurate)</SelectItem>
                                    <SelectItem value="localWhisper">🏠 Local Whisper (High Accuracy)</SelectItem>
                                    {/* <SelectItem value="deepgram">☁️ Deepgram (Backup)</SelectItem>
                                    <SelectItem value="elevenLabs">☁️ ElevenLabs</SelectItem>
                                    <SelectItem value="groq">☁️ Groq</SelectItem>
                                    */}
                                    <SelectItem value="openai">☁️ OpenAI</SelectItem>
                                </SelectContent>
                            </Select>

                            {!isLocalProvider(uiProvider) && (
                                <Select
                                    value={selectedCloudModel}
                                    onValueChange={(value) => {
                                        const model = value as string;
                                        const provider = uiProvider as CloudProvider;
                                        setTranscriptModelConfig({ provider, model, apiKey });
                                        void persistCloudConfig(provider, model, apiKey).catch((err) => {
                                            console.error('Failed to save transcript model config:', err);
                                        });
                                    }}
                                >
                                    <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                        <SelectValue placeholder="Select model" />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {cloudModelOptions.map((model) => (
                                            <SelectItem key={model} value={model}>{model}</SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                            )}

                        </div>
                    </div>

                    {uiProvider === 'localWhisper' && (
                        <div className="mt-6">
                            <ModelManager
                                selectedModel={transcriptModelConfig.provider === 'localWhisper' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleWhisperModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {uiProvider === 'parakeet' && (
                        <div className="mt-6">
                            <ParakeetModelManager
                                selectedModel={transcriptModelConfig.provider === 'parakeet' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleParakeetModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {uiProvider === 'qwenAsr' && (
                        <div className="mt-6">
                            <QwenAsrModelManager
                                selectedModel={transcriptModelConfig.provider === 'qwenAsr' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleQwenAsrModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {requiresApiKey && (
                        <div>
                            <Label className="block text-sm font-medium text-gray-700 mb-1">
                                API Key
                            </Label>
                            <div className="relative mx-1">
                                <Input
                                    type={showApiKey ? "text" : "password"}
                                    className={`pr-24 focus:ring-1 focus:ring-blue-500 focus:border-blue-500 ${isApiKeyLocked ? 'bg-gray-100 cursor-not-allowed' : ''
                                        }`}
                                    value={apiKey || ''}
                                    onChange={(e) => {
                                        const nextApiKey = e.target.value;
                                        setApiKey(nextApiKey);
                                        if (!isLocalProvider(uiProvider)) {
                                            setTranscriptModelConfig({
                                                provider: uiProvider,
                                                model: selectedCloudModel,
                                                apiKey: nextApiKey,
                                            });
                                        }
                                    }}
                                    onBlur={() => {
                                        if (!isLocalProvider(uiProvider)) {
                                            void persistCloudConfig(uiProvider, selectedCloudModel, apiKey).catch((err) => {
                                                console.error('Failed to save transcript API key:', err);
                                            });
                                        }
                                    }}
                                    onKeyDown={(e) => {
                                        if (e.key === 'Enter' && !isLocalProvider(uiProvider)) {
                                            void persistCloudConfig(uiProvider, selectedCloudModel, apiKey).catch((err) => {
                                                console.error('Failed to save transcript API key:', err);
                                            });
                                        }
                                    }}
                                    disabled={isApiKeyLocked}
                                    onClick={handleInputClick}
                                    placeholder="Enter your API key"
                                />
                                {isApiKeyLocked && (
                                    <div
                                        onClick={handleInputClick}
                                        className="absolute inset-0 flex items-center justify-center bg-gray-100 bg-opacity-50 rounded-md cursor-not-allowed"
                                    />
                                )}
                                <div className="absolute inset-y-0 right-0 pr-1 flex items-center">
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setIsApiKeyLocked(!isApiKeyLocked)}
                                        className={`transition-colors duration-200 ${isLockButtonVibrating ? 'animate-vibrate text-red-500' : ''
                                            }`}
                                        title={isApiKeyLocked ? "Unlock to edit" : "Lock to prevent editing"}
                                    >
                                        {isApiKeyLocked ? <Lock className="h-4 w-4" /> : <Unlock className="h-4 w-4" />}
                                    </Button>
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setShowApiKey(!showApiKey)}
                                    >
                                        {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                    </Button>
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div >
    )
}
