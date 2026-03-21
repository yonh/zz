import { useState } from "react";
import {
  Save,
  RotateCcw,
  Download,
  CheckCircle2,
  AlertCircle,
  FileText,
  Eye,
  EyeOff,
} from "lucide-react";
import { toast } from "sonner";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useAppStore } from "@/stores/store";

/**
 * Config page with TOML editor and validation.
 */
export default function Config() {
  const configToml = useAppStore((s) => s.configToml);
  const [localConfig, setLocalConfig] = useState(configToml);
  const [isValid, setIsValid] = useState(true);
  const [savedMsg, setSavedMsg] = useState(false);
  const [showKeys, setShowKeys] = useState(false);
  const [lastModified, setLastModified] = useState(new Date());
  const [lastReloaded, setLastReloaded] = useState<Date | null>(null);

  const isDirty = localConfig !== configToml;

  /**
   * Mask API keys in TOML content.
   */
  function maskApiKeys(text: string): string {
    return text.replace(
      /api_key\s*=\s*"([^"]+)"/g,
      (_, key) => `api_key = "${key.slice(0, 5)}****${key.slice(-4)}"`
    );
  }

  const displayConfig = showKeys ? localConfig : maskApiKeys(localConfig);

  /**
   * Simple TOML validation heuristic (mock).
   */
  function validateConfig(text: string): boolean {
    if (!text.trim()) return false;
    if (!text.includes("[server]")) return false;
    if (!text.includes("[[providers]]")) return false;
    return true;
  }

  /**
   * Handle config text change with validation.
   */
  function handleChange(value: string) {
    setLocalConfig(value);
    setIsValid(validateConfig(value));
    setSavedMsg(false);
    setLastModified(new Date());
  }

  /**
   * Simulate saving and reloading config.
   */
  function handleSave() {
    if (!isValid) return;
    setSavedMsg(true);
    setLastReloaded(new Date());
    toast.success("Config saved & reloaded");
    setTimeout(() => setSavedMsg(false), 3000);
  }

  /**
   * Reset editor to last saved config.
   */
  function handleReset() {
    setLocalConfig(configToml);
    setIsValid(true);
    setSavedMsg(false);
    toast.info("Config reset to last saved version");
  }

  /**
   * Download config as .toml file.
   */
  function handleDownload() {
    const blob = new Blob([localConfig], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "config.toml";
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Configuration</h1>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <FileText className="h-4 w-4" />
              config.toml
            </CardTitle>
            <div className="flex items-center gap-2">
              {isValid ? (
                <Badge variant="success" className="gap-1">
                  <CheckCircle2 className="h-3 w-3" />
                  Valid
                </Badge>
              ) : (
                <Badge variant="danger" className="gap-1">
                  <AlertCircle className="h-3 w-3" />
                  Invalid
                </Badge>
              )}
              {isDirty && (
                <Badge variant="warning">Unsaved changes</Badge>
              )}
              {savedMsg && (
                <Badge variant="success" className="gap-1">
                  <CheckCircle2 className="h-3 w-3" />
                  Saved & Reloaded
                </Badge>
              )}
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={() => setShowKeys(!showKeys)}
                title={showKeys ? "Hide API keys" : "Show API keys"}
              >
                {showKeys ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <Textarea
            value={displayConfig}
            onChange={(e) => handleChange(e.target.value)}
            className="min-h-[500px] font-mono text-sm leading-relaxed resize-y"
            spellCheck={false}
          />

          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Button
                onClick={handleSave}
                disabled={!isValid || !isDirty}
                className="gap-2"
              >
                <Save className="h-4 w-4" /> Save & Reload
              </Button>
              <Button
                variant="outline"
                onClick={handleReset}
                disabled={!isDirty}
                className="gap-2"
              >
                <RotateCcw className="h-4 w-4" /> Reset
              </Button>
              <Button
                variant="outline"
                onClick={handleDownload}
                className="gap-2"
              >
                <Download className="h-4 w-4" /> Download
              </Button>
            </div>

            <div className="text-xs text-muted-foreground space-y-1 text-right">
              <div>Lines: {localConfig.split("\n").length}</div>
              <div>Size: {new Blob([localConfig]).size} bytes</div>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="pt-6">
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <span>Config File Path: ~/.config/zz/config.toml</span>
            <div className="space-x-4">
              <span>Last modified: {lastModified.toLocaleString()}</span>
              {lastReloaded && (
                <span>Last reloaded: {lastReloaded.toLocaleString()}</span>
              )}
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
