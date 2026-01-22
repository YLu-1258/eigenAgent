// src/utils/format.ts

export function uid(): string {
    return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function formatTimestamp(timestamp: number): string {
    const now = new Date();
    const date = new Date(timestamp);
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    const timeStr = date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });

    // Less than 1 minute
    if (diffMins < 1) {
        return "Just now";
    }
    // Less than 1 hour
    if (diffMins < 60) {
        return `${diffMins} min ago`;
    }
    // Less than 24 hours
    if (diffHours < 24) {
        return `${diffHours} ${diffHours === 1 ? "hour" : "hours"} ago`;
    }
    // Yesterday
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    if (date.toDateString() === yesterday.toDateString()) {
        return `Yesterday at ${timeStr}`;
    }
    // Within last 7 days
    if (diffDays < 7) {
        const dayName = date.toLocaleDateString(undefined, { weekday: "long" });
        return `${dayName} at ${timeStr}`;
    }
    // Older - show date with time
    const dateStr = date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
    return `${dateStr} at ${timeStr}`;
}

export function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

export function formatSpeed(bps: number): string {
    return formatBytes(bps) + "/s";
}
