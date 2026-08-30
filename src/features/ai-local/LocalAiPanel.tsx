import { useEffect } from "react";
import { useLocalAiRuntime } from "./useLocalAiRuntime";
import { LocalModelList } from "./LocalModelList";
import { LocalRuntimeStatusView } from "./LocalRuntimeStatus";
import { LocalGenerateTest } from "./LocalGenerateTest";
import { docExtractAllText } from "../../lib/ipc";
import type { LocalGenerateRequest } from "./types";

interface LocalAiPanelProps {
  sessionId: string;
  documentTitle: string;
  onAcceptGeneratedText?: (text: string) => Promise<boolean>;
}

export const LocalAiPanel = ({ sessionId, documentTitle, onAcceptGeneratedText }: LocalAiPanelProps) => {
  const runtime = useLocalAiRuntime();

  useEffect(() => {
    void runtime.refreshStatus();
    void runtime.refreshModels();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const generateForDocument = async (request: LocalGenerateRequest) => {
    const extraction = await docExtractAllText(sessionId);
    if (!extraction.ok) {
      runtime.setErrorMessage(`Document context extraction failed: ${extraction.error.message}`);
      return null;
    }
    const context = extraction.data
      .map((text, index) => text.trim() ? `Page ${index + 1}: ${text.trim()}` : "")
      .filter(Boolean)
      .join("\n")
      .slice(0, 12_000);
    const prompt = `${request.prompt}\n\nActive document: ${documentTitle}\nDocument context:\n${context || "(The active document has no extractable text.)"}`;
    return runtime.generate({ ...request, prompt });
  };

  return (
    <div className="local-ai-panel">
      <h4>Local AI Runtime</h4>
      <LocalRuntimeStatusView status={runtime.status} />
      <LocalModelList models={runtime.models} onValidate={runtime.validateModel} />
      <LocalGenerateTest
        models={runtime.models}
        generating={runtime.generating}
        lastResult={runtime.lastResult}
        error={runtime.error}
        onGenerate={generateForDocument}
        runtimeAvailable={runtime.status?.available ?? false}
        documentLabel={documentTitle}
        onAccept={onAcceptGeneratedText}
        onReject={runtime.clearLastResult}
      />
    </div>
  );
};
