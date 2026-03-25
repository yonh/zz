import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

/**
 * Merge Tailwind CSS classes with clsx and tailwind-merge.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Format duration in milliseconds to human-readable string.
 * Always uses seconds as minimum unit.
 * @param ms - Duration in milliseconds
 * @returns Formatted string (e.g., "0.15s", "1.5s", "2m30s", "1h15m")
 */
export function formatDuration(ms: number): string {
  if (ms < 0) return "0s";

  const seconds = ms / 1000;

  // Less than 1 minute - show in seconds
  if (seconds < 60) {
    return `${seconds.toFixed(seconds < 10 ? 2 : 1)}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Math.round(seconds % 60);

  // Less than 1 hour - show minutes and seconds
  if (minutes < 60) {
    return `${minutes}m${remainingSeconds}s`;
  }

  // 1 hour or more - show hours and minutes
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h${remainingMinutes}m`;
}
