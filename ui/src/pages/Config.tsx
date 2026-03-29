import { useState, useEffect } from "react";
import {
  Save,
  RotateCcw,
  Download,
  CheckCircle2,
  AlertCircle,
  FileText,
  Eye,
  EyeOff,
  Loader2,
} from "lucide-react";
import { toast } from "sonner";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/api/client";

export default function Config() {
  const [localConfig, setLocalConfig] = useState("");
  const [originalConfig, setOriginalConfig] = useState("");
  const [isValid, setIsValid] = useState(true);
  const [savedMsg, setSavedMsg] = useState(false);
  const [showKeys, setShowKeys] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [lastReloaded, setLastReloaded] = useState<string | null>(null);

  const isDirty = localConfig !== originalConfig;

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    setLoading(true);
    try {
      const response = await api.getConfig();
      setLocalConfig(response.content);
      setOriginalConfig(response.content);
    } catch (error) {
      toast.error("Failed to load config");
    } finally {
      setLoading(false);
    }
  }

  function maskApiKeys(text: string): string {
    return text.replace(
      /api_key\s*=\s*"([^"]+)"/g,
      (_, key) => `api_key = "${key.slice(0, 5)}****${key.slice(-4)}"`
    );
  }

  const displayConfig = showKeys ? localConfig : maskApiKeys(localConfig);

  async function handleSave() {
    if (!isValid || !isDirty) return;
    
    setSaving(true);
    try {
      const validation = await api.validateConfig(localConfig);
      if (!validation.valid) {
        toast.error(validation.errors?.join("\n") || "Invalid config");
        setIsValid(false);
        return;
      }
      
      await api.updateConfig(localConfig);
      setOriginalConfig(localConfig);
      setSavedMsg(true);
      setLastReloaded(new Date().toLocaleString());
      toast.success("Config saved & reloaded");
      setTimeout(() => setSavedMsg(false), 3000);
    } catch (error) {
      toast.error("Failed to save config");
    } finally {
      setSaving(false);
    }
  }

  function handleReset() {
    setLocalConfig(originalConfig);
    setIsValid(true);
    setSavedMsg(false);
    toast.info("Config reset to last saved version");
  }

  function handleDownload() {
    const blob = new Blob([localConfig], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "config.toml";
    a.click();
    URL.revokeObjectURL(url);
  }

  function handleChange(value: string) {
    setLocalConfig(value);
    setSavedMsg(false);
    if (value.includes("[server]") && value.includes("[[providers]]")) {
      setIsValid(true);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
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
                disabled={!isValid || !isDirty || saving}
                className="gap-2"
              >
                {saving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                Save & Reload
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
            {lastReloaded && (
              <span>Last reloaded: {lastReloaded}</span>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
