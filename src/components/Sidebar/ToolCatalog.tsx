// src/components/Sidebar/ToolCatalog.tsx

import { ToolWithStatus, TOOL_ICONS, CATEGORY_NAMES, ToolCategory } from "../../types/tools";

interface ToolCatalogProps {
    tools: ToolWithStatus[];
    onClose: () => void;
    onToggleTool: (toolId: string, enabled: boolean) => void;
}

export function ToolCatalog({ tools, onClose, onToggleTool }: ToolCatalogProps) {
    // Group tools by category
    const groupedTools = tools.reduce((acc, tool) => {
        const category = tool.category as ToolCategory;
        if (!acc[category]) {
            acc[category] = [];
        }
        acc[category].push(tool);
        return acc;
    }, {} as Record<ToolCategory, ToolWithStatus[]>);

    // Order of categories
    const categoryOrder: ToolCategory[] = ["search", "web", "filesystem", "system"];

    return (
        <div className="toolCatalog">
            <div className="toolCatalogHeader">
                <span>Tools</span>
                <button className="closeCatalogBtn" onClick={onClose}>
                    <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                    >
                        <path d="M18 6L6 18M6 6l12 12" />
                    </svg>
                </button>
            </div>
            <div className="toolList niceScroll">
                {tools.length === 0 ? (
                    <div className="toolListEmpty">No tools available</div>
                ) : (
                    categoryOrder.map((category) => {
                        const categoryTools = groupedTools[category];
                        if (!categoryTools || categoryTools.length === 0) return null;

                        return (
                            <div key={category} className="toolCategory">
                                <div className="toolCategoryHeader">
                                    {CATEGORY_NAMES[category]}
                                </div>
                                {categoryTools.map((tool) => (
                                    <div
                                        key={tool.id}
                                        className={`toolItem ${tool.enabled ? "enabled" : ""}`}
                                    >
                                        <div className="toolItemMain">
                                            <span className="toolIcon">
                                                {TOOL_ICONS[tool.icon] || "🔧"}
                                            </span>
                                            <div className="toolInfo">
                                                <div className="toolName">
                                                    {tool.name}
                                                    {tool.requiresConfirmation && (
                                                        <span
                                                            className="warningBadge"
                                                            title="Requires confirmation for dangerous operations"
                                                        >
                                                            ⚠️
                                                        </span>
                                                    )}
                                                </div>
                                                <div className="toolDescription">
                                                    {tool.description}
                                                </div>
                                            </div>
                                        </div>
                                        <label className="toolToggle">
                                            <input
                                                type="checkbox"
                                                checked={tool.enabled}
                                                onChange={(e) =>
                                                    onToggleTool(tool.id, e.target.checked)
                                                }
                                            />
                                            <span className="toggleSlider"></span>
                                        </label>
                                    </div>
                                ))}
                            </div>
                        );
                    })
                )}
            </div>
        </div>
    );
}
