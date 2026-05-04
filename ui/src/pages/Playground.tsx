import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  Loader2,
  RotateCcw,
  Send,
  Terminal,
} from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useAppStore } from "@/stores/store";
import type { Provider } from "@/api/types";
import { cn } from "@/lib/utils";

interface TestResult {
  ok: boolean;
  status?: number;
  durationMs?: number;
  body: string;
  error?: string;
}

const DEFAULT_AUTH_TOKEN = "zz-playground";
const DEFAULT_PROMPT = "hi";
const ENDPOINT_PRESETS = ["/v1/chat/completions", "/v1/responses", "/v1/messages"];

/**
 * Get the default proxy URL from the current browser location.
 */
function getDefaultProxyBaseUrl(): string {
  const { hostname, origin, protocol, port } = window.location;
  const localHosts = new Set(["localhost", "127.0.0.1", "0.0.0.0"]);
  if (localHosts.has(hostname) && port && port !== "9090") {
    return `http://${hostname}:9090`;
  }
  if (origin === "null") {
    return `${protocol}//${hostname}:9090`;
  }
  return origin;
}

/**
 * Trim trailing slashes from a proxy base URL.
 */
function normalizeBaseUrl(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

/**
 * Normalize an endpoint path to an absolute HTTP path.
 */
function normalizeEndpointPath(value: string): string {
  const trimmed = value.trim() || "/v1/chat/completions";
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
}

/**
 * Combine a proxy base URL and endpoint path.
 */
function buildRequestUrl(baseUrl: string, endpointPath: string): string {
  return `${normalizeBaseUrl(baseUrl)}${normalizeEndpointPath(endpointPath)}`;
}

/**
 * Escape a value for safe single-quoted shell usage.
 */
function shellSingleQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

/**
 * Build a copy-ready curl command for the configured request.
 */
function buildCurl(baseUrl: string, endpointPath: string, authToken: string, body: string): string {
  return [
    `curl -sS ${shellSingleQuote(buildRequestUrl(baseUrl, endpointPath))} \\`,
    `  -H ${shellSingleQuote(`Authorization: Bearer ${authToken || DEFAULT_AUTH_TOKEN}`)} \\`,
    `  -H ${shellSingleQuote("Content-Type: application/json")} \\`,
    `  -d ${shellSingleQuote(body)}`,
  ].join("\n");
}

/**
 * Format a JavaScript value as stable pretty JSON.
 */
function formatJson(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

/**
 * Create a default OpenAI-compatible chat completion request body.
 */
function createRequestBody(model: string, prompt: string, temperature: string, maxTokens: string, stream: boolean): string {
  const parsedTemperature = Number(temperature);
  const parsedMaxTokens = Number(maxTokens);
  const payload: Record<string, unknown> = {
    model,
    messages: [
      {
        role: "user",
        content: prompt,
      },
    ],
    temperature: Number.isFinite(parsedTemperature) ? parsedTemperature : 0.2,
    stream,
  };

  if (Number.isFinite(parsedMaxTokens) && parsedMaxTokens > 0) {
    payload.max_tokens = parsedMaxTokens;
  }

  return formatJson(payload);
}

/**
 * Pretty-print JSON response text when possible.
 */
function formatResponseBody(text: string): string {
  if (!text) return "(empty response)";
  try {
    return formatJson(JSON.parse(text));
  } catch {
    return text;
  }
}

/**
 * Convert a model glob pattern into a regular expression.
 */
function patternToRegex(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\*/g, ".*").replace(/\?/g, ".")}$`);
}

/**
 * Check whether a provider's model patterns match a model name.
 */
function providerSupportsModel(provider: Provider, model: string): boolean {
  if (provider.models.length === 0) return true;
  return provider.models.some((pattern) => patternToRegex(pattern).test(model));
}

/**
 * Collect unique configured model patterns from all providers.
 */
function getUniqueModels(providers: Provider[]): string[] {
  return Array.from(new Set(providers.flatMap((provider) => provider.models))).sort((a, b) => a.localeCompare(b));
}

/**
 * Decide whether browser testing can use a relative URL.
 */
function shouldUseRelativeRequest(proxyBaseUrl: string): boolean {
  const normalizedProxy = normalizeBaseUrl(proxyBaseUrl);
  const normalizedCurrent = normalizeBaseUrl(window.location.origin);
  if (normalizedProxy === normalizedCurrent) return true;

  const { hostname, port } = window.location;
  const localHosts = new Set(["localhost", "127.0.0.1", "0.0.0.0"]);
  return localHosts.has(hostname) && Boolean(port) && port !== "9090" && normalizedProxy === `http://${hostname}:9090`;
}

/**
 * Build the URL used by browser fetch, accounting for Vite proxy mode.
 */
function buildBrowserRequestUrl(proxyBaseUrl: string, endpointPath: string): string {
  if (shouldUseRelativeRequest(proxyBaseUrl)) {
    return normalizeEndpointPath(endpointPath);
  }
  return buildRequestUrl(proxyBaseUrl, endpointPath);
}

/**
 * Copy text with a clipboard API fallback.
 */
async function copyText(value: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }

  const el = document.createElement("textarea");
  el.value = value;
  el.style.position = "fixed";
  el.style.opacity = "0";
  document.body.appendChild(el);
  el.focus();
  el.select();
  document.execCommand("copy");
  document.body.removeChild(el);
}

/**
 * Playground page for generating curl commands and smoke testing proxy requests.
 */
export default function Playground() {
  const providers = useAppStore((state) => state.providers);
  const modelOptions = useMemo(() => getUniqueModels(providers), [providers]);
  const defaultModel = modelOptions[0] || "gpt-4o-mini";
  const [proxyBaseUrl, setProxyBaseUrl] = useState(getDefaultProxyBaseUrl);
  const [endpointPath, setEndpointPath] = useState("/v1/chat/completions");
  const [authToken, setAuthToken] = useState(DEFAULT_AUTH_TOKEN);
  const [model, setModel] = useState(defaultModel);
  const [prompt, setPrompt] = useState(DEFAULT_PROMPT);
  const [temperature, setTemperature] = useState("1");
  const [maxTokens, setMaxTokens] = useState("512");
  const [stream, setStream] = useState(false);
  const [requestBody, setRequestBody] = useState(() =>
    createRequestBody(defaultModel, DEFAULT_PROMPT, "1", "512", false)
  );
  const [isTesting, setIsTesting] = useState(false);
  const [result, setResult] = useState<TestResult | null>(null);

  useEffect(() => {
    if (modelOptions.length === 0 || model !== "gpt-4o-mini") return;
    handleModelChange(modelOptions[0]);
  }, [model, modelOptions]);

  const matchingProviders = useMemo(
    () => providers.filter((provider) => providerSupportsModel(provider, model)),
    [model, providers]
  );

  const activeMatchingProviders = matchingProviders.filter(
    (provider) => provider.enabled && provider.status !== "disabled"
  );

  const curl = useMemo(
    () => buildCurl(proxyBaseUrl, endpointPath, authToken, requestBody),
    [authToken, endpointPath, proxyBaseUrl, requestBody]
  );

  /**
   * Regenerate the editable request JSON from form fields.
   */
  function regenerateRequestBody(nextModel = model): void {
    setRequestBody(createRequestBody(nextModel, prompt, temperature, maxTokens, stream));
  }

  /**
   * Update the selected model and keep request JSON in sync.
   */
  function handleModelChange(value: string): void {
    setModel(value);
    setRequestBody(createRequestBody(value, prompt, temperature, maxTokens, stream));
  }

  /**
   * Toggle streaming and update request JSON.
   */
  function handleStreamToggle(): void {
    const next = !stream;
    setStream(next);
    setRequestBody(createRequestBody(model, prompt, temperature, maxTokens, next));
  }

  /**
   * Copy the generated curl command to the clipboard.
   */
  async function handleCopyCurl(): Promise<void> {
    try {
      await copyText(curl);
      toast.success("curl copied");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to copy curl");
    }
  }

  /**
   * Send a browser-based smoke test through the proxy.
   */
  async function handleSendTest(): Promise<void> {
    let parsedBody: unknown;
    try {
      parsedBody = JSON.parse(requestBody);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Request body is not valid JSON");
      return;
    }

    const startedAt = performance.now();
    setIsTesting(true);
    setResult(null);
    try {
      const response = await fetch(buildBrowserRequestUrl(proxyBaseUrl, endpointPath), {
        method: "POST",
        headers: {
          Authorization: `Bearer ${authToken || DEFAULT_AUTH_TOKEN}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(parsedBody),
      });
      const body = await response.text();
      const durationMs = Math.round(performance.now() - startedAt);
      setResult({
        ok: response.ok,
        status: response.status,
        durationMs,
        body: formatResponseBody(body),
      });
      if (response.ok) {
        toast.success(`Test completed in ${durationMs}ms`);
      } else {
        toast.error(`Test failed with HTTP ${response.status}`);
      }
    } catch (err) {
      const durationMs = Math.round(performance.now() - startedAt);
      const message = err instanceof Error ? err.message : "Request failed";
      setResult({ ok: false, durationMs, body: "", error: message });
      toast.error(message);
    } finally {
      setIsTesting(false);
    }
  }

  return (
    <div className="space-y-6 overflow-auto pb-8">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <div className="flex items-center gap-2">
            <Terminal className="h-5 w-5 text-chart-1" />
            <h1 className="text-2xl font-bold tracking-tight">Playground</h1>
          </div>
          <p className="mt-1 text-sm text-muted-foreground">
            Generate copy-ready proxy curl commands and run quick model smoke tests.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant="secondary">{providers.length} providers</Badge>
          <Badge variant={activeMatchingProviders.length > 0 ? "success" : "warning"}>
            {activeMatchingProviders.length} active match
          </Badge>
        </div>
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(420px,0.9fr)]">
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Request target</CardTitle>
              <CardDescription>Point the test at your ZZ proxy, not the upstream provider directly.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(220px,0.45fr)]">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Proxy base URL</label>
                  <Input value={proxyBaseUrl} onChange={(event) => setProxyBaseUrl(event.target.value)} />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Authorization placeholder</label>
                  <Input value={authToken} onChange={(event) => setAuthToken(event.target.value)} />
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Endpoint path</label>
                <Input value={endpointPath} onChange={(event) => setEndpointPath(event.target.value)} />
                <div className="flex flex-wrap gap-2">
                  {ENDPOINT_PRESETS.map((preset) => (
                    <Button key={preset} type="button" variant="outline" size="sm" onClick={() => setEndpointPath(preset)}>
                      {preset}
                    </Button>
                  ))}
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Model smoke test</CardTitle>
              <CardDescription>Pick a configured model or type a custom one, then edit the JSON if needed.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_120px_120px_120px]">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Model</label>
                  <Input list="zz-playground-models" value={model} onChange={(event) => handleModelChange(event.target.value)} />
                  <datalist id="zz-playground-models">
                    {modelOptions.map((option) => (
                      <option key={option} value={option} />
                    ))}
                  </datalist>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Temperature</label>
                  <Input value={temperature} onChange={(event) => setTemperature(event.target.value)} onBlur={() => regenerateRequestBody()} />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Max tokens</label>
                  <Input value={maxTokens} onChange={(event) => setMaxTokens(event.target.value)} onBlur={() => regenerateRequestBody()} />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Stream</label>
                  <Button type="button" variant={stream ? "default" : "outline"} className="w-full" onClick={handleStreamToggle}>
                    {stream ? "Enabled" : "Disabled"}
                  </Button>
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">Prompt</label>
                <Textarea
                  className="min-h-[90px] font-sans"
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  onBlur={() => regenerateRequestBody()}
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between gap-2">
                  <label className="text-sm font-medium">Request JSON</label>
                  <Button type="button" variant="ghost" size="sm" className="gap-1.5" onClick={() => regenerateRequestBody()}>
                    <RotateCcw className="h-3.5 w-3.5" /> Regenerate
                  </Button>
                </div>
                <Textarea className="min-h-[260px]" value={requestBody} onChange={(event) => setRequestBody(event.target.value)} />
              </div>
            </CardContent>
          </Card>
        </div>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Generated curl</CardTitle>
              <CardDescription>Copy this command into a terminal to test the same request outside the browser.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <pre className="max-h-[360px] overflow-auto rounded-lg border bg-muted p-4 text-xs leading-relaxed text-foreground">
                <code>{curl}</code>
              </pre>
              <div className="flex flex-wrap gap-2">
                <Button type="button" className="gap-2" onClick={handleCopyCurl}>
                  <Copy className="h-4 w-4" /> Copy curl
                </Button>
                <Button type="button" variant="outline" className="gap-2" onClick={handleSendTest} disabled={isTesting}>
                  {isTesting ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                  Send test
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Routing preview</CardTitle>
              <CardDescription>Providers whose configured model patterns match the selected model.</CardDescription>
            </CardHeader>
            <CardContent>
              {matchingProviders.length === 0 ? (
                <div className="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-700 dark:text-amber-300">
                  <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                  <span>No configured provider explicitly matches this model.</span>
                </div>
              ) : (
                <div className="space-y-2">
                  {matchingProviders.map((provider) => (
                    <div key={provider.name} className="flex items-center justify-between gap-3 rounded-lg border p-3">
                      <div className="min-w-0">
                        <div className="truncate text-sm font-medium">{provider.name}</div>
                        <div className="truncate text-xs text-muted-foreground">
                          {provider.models.length > 0 ? provider.models.join(", ") : "all models"}
                        </div>
                      </div>
                      <Badge variant={provider.enabled && provider.status !== "disabled" ? "success" : "secondary"}>
                        {provider.status}
                      </Badge>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Test result</CardTitle>
              <CardDescription>Result from the browser test request through the proxy.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {!result ? (
                <div className="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground">
                  No test has been sent yet.
                </div>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-2">
                    <Badge variant={result.ok ? "success" : "danger"} className="gap-1.5">
                      {result.ok ? <CheckCircle2 className="h-3.5 w-3.5" /> : <AlertTriangle className="h-3.5 w-3.5" />}
                      {result.status ? `HTTP ${result.status}` : "Request error"}
                    </Badge>
                    {result.durationMs !== undefined && <Badge variant="outline">{result.durationMs}ms</Badge>}
                  </div>
                  {result.error && <div className="rounded-lg bg-destructive/10 p-3 text-sm text-destructive">{result.error}</div>}
                  <pre
                    className={cn(
                      "max-h-[420px] overflow-auto rounded-lg border bg-muted p-4 text-xs leading-relaxed",
                      !result.body && "hidden"
                    )}
                  >
                    <code>{result.body}</code>
                  </pre>
                </>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
