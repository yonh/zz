import { ArrowRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { formatDuration } from "@/lib/utils";
import type { LogEntry } from "@/api/types";

/**
 * Expandable log detail panel used in Logs page and Overview modal.
 */
export function LogDetailPanel({ log }: { log: LogEntry }) {
  return (
    <div className="px-4 py-3 bg-muted/30 border-t text-xs space-y-2">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div>
          <span className="text-muted-foreground">Request ID</span>
          <div className="font-mono mt-0.5">{log.id}</div>
        </div>
        <div>
          <span className="text-muted-foreground">Duration</span>
          <div className="font-mono mt-0.5">
            {formatDuration(log.duration_ms)} (TTFB: {formatDuration(log.ttfb_ms)})
          </div>
        </div>
        <div>
          <span className="text-muted-foreground">Model</span>
          <div className="font-mono mt-0.5">{log.model}</div>
        </div>
        <div>
          <span className="text-muted-foreground">Streaming</span>
          <div className="mt-0.5">{log.streaming ? "Yes" : "No"}</div>
        </div>
        <div>
          <span className="text-muted-foreground">Request Size</span>
          <div className="font-mono mt-0.5">
            {(log.request_bytes / 1024).toFixed(1)} KB
          </div>
        </div>
        <div>
          <span className="text-muted-foreground">Response Size</span>
          <div className="font-mono mt-0.5">
            {(log.response_bytes / 1024).toFixed(1)} KB
          </div>
        </div>
        {log.failover_chain && (
          <div className="col-span-2">
            <span className="text-muted-foreground">Failover Chain</span>
            <div className="flex items-center gap-1 mt-0.5 font-mono">
              {log.failover_chain.map((step, i) => (
                <span key={i} className="flex items-center gap-1">
                  {i > 0 && <ArrowRight className="h-3 w-3 text-amber-500" />}
                  <Badge
                    variant={step.includes("200") ? "success" : "warning"}
                    className="text-[10px]"
                  >
                    {step}
                  </Badge>
                </span>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
