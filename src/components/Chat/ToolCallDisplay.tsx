// src/components/Chat/ToolCallDisplay.tsx

import { ToolCallDisplay as ToolCallDisplayType, TOOL_ICONS } from "../../types/tools";

interface ToolCallDisplayProps {
    toolCalls: ToolCallDisplayType[];
}

export function ToolCallDisplay({ toolCalls }: ToolCallDisplayProps) {
    if (toolCalls.length === 0) return null;

    return (
        <div className="toolCallsContainer">
            {toolCalls.map((call) => (
                <div key={call.id} className={`toolCallCard ${call.status}`}>
                    <div className="toolCallHeader">
                        <span className="toolCallIcon">
                            {getIconForTool(call.toolId)}
                        </span>
                        <span className="toolCallName">
                            {getToolDisplayName(call.toolName)}
                        </span>
                        <span className={`toolCallStatus ${call.status}`}>
                            {getStatusIcon(call.status)}
                        </span>
                    </div>

                    {Object.keys(call.arguments).length > 0 && (
                        <div className="toolCallArgs">
                            {formatArguments(call.arguments)}
                        </div>
                    )}

                    {call.status === "success" && call.output && (
                        <div className="toolCallOutput">
                            <div className="toolCallOutputHeader">Result</div>
                            <div className="toolCallOutputContent">
                                {truncateOutput(call.output)}
                            </div>
                        </div>
                    )}

                    {call.status === "error" && call.error && (
                        <div className="toolCallError">
                            {call.error}
                        </div>
                    )}
                </div>
            ))}
        </div>
    );
}

function getIconForTool(toolId: string): string {
    const iconMap: Record<string, string> = {
        wikipedia: TOOL_ICONS.book,
        web_search: TOOL_ICONS.globe,
        filesystem: TOOL_ICONS.folder,
        shell: TOOL_ICONS.terminal,
        calculator: TOOL_ICONS.calculator,
    };
    return iconMap[toolId] || "🔧";
}

function getToolDisplayName(toolName: string): string {
    const nameMap: Record<string, string> = {
        wikipedia: "Wikipedia",
        web_search: "Web Search",
        filesystem: "File System",
        shell: "Shell",
        calculator: "Calculator",
    };
    return nameMap[toolName] || toolName;
}

function getStatusIcon(status: string): string {
    switch (status) {
        case "pending":
            return "⏳";
        case "running":
            return "⚡";
        case "success":
            return "✓";
        case "error":
            return "✗";
        default:
            return "•";
    }
}

function formatArguments(args: Record<string, unknown>): string {
    const parts: string[] = [];
    for (const [key, value] of Object.entries(args)) {
        if (typeof value === "string") {
            // Truncate long strings
            const displayValue = value.length > 50 ? value.slice(0, 50) + "..." : value;
            parts.push(`${key}: "${displayValue}"`);
        } else {
            parts.push(`${key}: ${JSON.stringify(value)}`);
        }
    }
    return parts.join(", ");
}

function truncateOutput(output: string): string {
    const maxLength = 500;
    if (output.length <= maxLength) return output;
    return output.slice(0, maxLength) + "\n... (truncated)";
}
