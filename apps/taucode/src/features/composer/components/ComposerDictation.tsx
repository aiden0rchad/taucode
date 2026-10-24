import { useEffect, useRef } from "react";
import { Mic, Square, X } from "lucide-react";
import type { AppSettings } from "@/types";
import { useDictationController } from "@app/hooks/useDictationController";
import { useComposerDictationControls } from "../hooks/useComposerDictationControls";

type Props = {
  settings: AppSettings;
  disabled: boolean;
  onTranscript: (text: string) => void;
  onBusyChange: (busy: boolean) => void;
  onError: (error: string) => void;
  onOpenSettings: () => void;
};

export function ComposerDictation(props: Props) {
  if (!props.settings.dictationEnabled) return <button type="button" className="composer-action composer-action--mic" aria-label="Open dictation settings" title="Dictation disabled. Open settings" onClick={props.onOpenSettings}><Mic aria-hidden /></button>;
  return <EnabledDictation {...props} />;
}

function EnabledDictation({ settings, disabled, onTranscript, onBusyChange, onError, onOpenSettings }: Props) {
  const dictation = useDictationController({ ...settings, dictationEnabled: !disabled });
  const controls = useComposerDictationControls({
    disabled: disabled && dictation.dictationState === "idle",
    dictationEnabled: dictation.dictationReady,
    dictationState: dictation.dictationState,
    onToggleDictation: () => { void dictation.handleToggleDictation(); },
    onCancelDictation: () => { void dictation.cancelDictation().catch(error => onError(String(error))); },
    onOpenDictationSettings: onOpenSettings,
  });
  const consumed = useRef<string | null>(null);
  const latest = useRef(dictation);
  latest.current = dictation;
  useEffect(() => {
    const transcript = dictation.dictationTranscript;
    if (!transcript || consumed.current === transcript.id) return;
    consumed.current = transcript.id;
    onTranscript(transcript.text);
    dictation.clearDictationTranscript(transcript.id);
  }, [dictation, onTranscript]);
  useEffect(() => { onBusyChange(controls.isDictationBusy); }, [controls.isDictationBusy, onBusyChange]);
  useEffect(() => { if (dictation.dictationError) onError(dictation.dictationError); }, [dictation.dictationError, onError]);
  useEffect(() => () => {
    if (latest.current.dictationState !== "idle") void latest.current.cancelDictation().catch(() => {});
    onBusyChange(false);
  }, [onBusyChange]);
  return <button type="button" className={`composer-action composer-action--mic${controls.isDictationBusy ? " is-active" : ""}`} aria-label={controls.micAriaLabel} title={controls.micTitle} disabled={controls.micDisabled} onClick={controls.handleMicClick}>
    {controls.isDictationProcessing ? <X aria-hidden /> : controls.isDictating ? <Square aria-hidden /> : <Mic aria-hidden />}
  </button>;
}
