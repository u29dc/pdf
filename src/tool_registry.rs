use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
pub struct ToolCatalogPayload {
    pub version: &'static str,
    #[serde(rename = "globalFlags")]
    pub global_flags: Vec<GlobalFlag>,
    pub tools: Vec<ToolDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDetailPayload {
    pub version: &'static str,
    #[serde(rename = "globalFlags")]
    pub global_flags: Vec<GlobalFlag>,
    pub tool: ToolDescriptor,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalFlag {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub description: &'static str,
    pub default: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDescriptor {
    pub name: &'static str,
    pub command: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub parameters: Vec<ParameterDescriptor>,
    #[serde(rename = "outputFields")]
    pub output_fields: Vec<FieldDescriptor>,
    #[serde(rename = "outputSchema")]
    pub output_schema: Value,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    pub idempotent: bool,
    #[serde(rename = "rateLimit")]
    pub rate_limit: Option<&'static str>,
    pub example: ToolExample,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParameterDescriptor {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub required: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldDescriptor {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolExample {
    pub command: &'static str,
    pub description: &'static str,
}

pub fn catalog_payload() -> ToolCatalogPayload {
    ToolCatalogPayload {
        version: env!("CARGO_PKG_VERSION"),
        global_flags: global_flags(),
        tools: tool_registry(),
    }
}

pub fn detail_payload(name: &str) -> Option<ToolDetailPayload> {
    let tool = tool_registry().into_iter().find(|tool| tool.name == name)?;
    Some(ToolDetailPayload {
        version: env!("CARGO_PKG_VERSION"),
        global_flags: global_flags(),
        tool,
    })
}

fn global_flags() -> Vec<GlobalFlag> {
    vec![
        GlobalFlag {
            name: "--text",
            value_type: "boolean",
            description: "Emit human-readable output instead of the default JSON envelope.",
            default: json!(false),
        },
        GlobalFlag {
            name: "--help",
            value_type: "boolean",
            description: "Print command help for the current scope.",
            default: json!(false),
        },
        GlobalFlag {
            name: "--version",
            value_type: "boolean",
            description: "Print the installed pdf version.",
            default: json!(false),
        },
    ]
}

fn tool_registry() -> Vec<ToolDescriptor> {
    let mut tools = vec![tools_tool(), optimize_tool()];
    tools.sort_by(|left, right| left.category.cmp(right.category).then(left.name.cmp(right.name)));
    tools
}

fn tools_tool() -> ToolDescriptor {
    ToolDescriptor {
        name: "pdf.tools",
        command: "pdf [--text] tools [name]",
        category: "introspection",
        description: "Describe the full CLI tool catalog or one tool in machine-discoverable detail.",
        parameters: vec![ParameterDescriptor {
            name: "name",
            value_type: "string",
            required: false,
            description: "Optional dotted tool name for detail mode, for example pdf.optimize.",
        }],
        output_fields: vec![
            FieldDescriptor {
                name: "version",
                value_type: "string",
                description: "CLI version string.",
            },
            FieldDescriptor {
                name: "globalFlags",
                value_type: "array",
                description: "Global flags available to every command.",
            },
            FieldDescriptor {
                name: "tools",
                value_type: "array",
                description: "Tool catalog in category/name order.",
            },
            FieldDescriptor {
                name: "tool",
                value_type: "object",
                description: "Single tool detail payload returned by `pdf tools <name>`.",
            },
        ],
        output_schema: json!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "version": { "type": "string" },
                        "globalFlags": { "type": "array", "items": global_flag_schema() },
                        "tools": { "type": "array", "items": tool_descriptor_schema() }
                    },
                    "required": ["version", "globalFlags", "tools"],
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "properties": {
                        "version": { "type": "string" },
                        "globalFlags": { "type": "array", "items": global_flag_schema() },
                        "tool": tool_descriptor_schema()
                    },
                    "required": ["version", "globalFlags", "tool"],
                    "additionalProperties": false
                }
            ]
        }),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Optional dotted tool name for detail mode."
                }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: None,
        example: ToolExample {
            command: "pdf tools pdf.optimize",
            description: "Fetch the full metadata contract for the optimize command.",
        },
    }
}

fn optimize_tool() -> ToolDescriptor {
    ToolDescriptor {
        name: "pdf.optimize",
        command: "pdf [--text] optimize <path> [--apply] [--estimate-size] [--min-size-savings-bytes <bytes>] [--min-size-savings-percent <percent>] [--jobs <n>] [--no-backup]",
        category: "pdf",
        description: "Scan one PDF or directory tree, normalize metadata, estimate size savings, and optionally apply changes in place.",
        parameters: vec![
            ParameterDescriptor {
                name: "path",
                value_type: "string",
                required: true,
                description: "Visible PDF file or directory path to scan.",
            },
            ParameterDescriptor {
                name: "--apply",
                value_type: "boolean",
                required: false,
                description: "Write optimized output back to each target file.",
            },
            ParameterDescriptor {
                name: "--estimate-size",
                value_type: "boolean",
                required: false,
                description: "Run qpdf-based size estimation during planning.",
            },
            ParameterDescriptor {
                name: "--min-size-savings-bytes",
                value_type: "integer",
                required: false,
                description: "Minimum byte savings required before optimized output replaces the source file.",
            },
            ParameterDescriptor {
                name: "--min-size-savings-percent",
                value_type: "number",
                required: false,
                description: "Minimum percentage savings required before optimized output replaces the source file.",
            },
            ParameterDescriptor {
                name: "--jobs",
                value_type: "integer",
                required: false,
                description: "Override the worker pool size. Must be at least 1.",
            },
            ParameterDescriptor {
                name: "--no-backup",
                value_type: "boolean",
                required: false,
                description: "Skip backup snapshot creation before apply mode writes.",
            },
        ],
        output_fields: vec![
            FieldDescriptor {
                name: "timestamp",
                value_type: "string",
                description: "Run timestamp used for report naming.",
            },
            FieldDescriptor {
                name: "mode",
                value_type: "string",
                description: "Run mode: `plan` or `apply`.",
            },
            FieldDescriptor {
                name: "targetPath",
                value_type: "string",
                description: "Resolved input path.",
            },
            FieldDescriptor {
                name: "options",
                value_type: "object",
                description: "Effective optimize command options.",
            },
            FieldDescriptor {
                name: "summary",
                value_type: "object",
                description: "Aggregate counts and size totals for the run.",
            },
            FieldDescriptor {
                name: "backupRoot",
                value_type: "string",
                description: "Backup snapshot root for apply mode or an empty string when unused.",
            },
            FieldDescriptor {
                name: "reportPath",
                value_type: "string",
                description: "Path to the persisted JSON report.",
            },
            FieldDescriptor {
                name: "files",
                value_type: "array",
                description: "Per-file plan and apply results.",
            },
        ],
        output_schema: json!({
            "type": "object",
            "properties": {
                "timestamp": { "type": "string" },
                "mode": { "type": "string", "enum": ["plan", "apply"] },
                "targetPath": { "type": "string" },
                "options": {
                    "type": "object",
                    "properties": {
                        "apply": { "type": "boolean" },
                        "estimateSize": { "type": "boolean" },
                        "minSizeSavingsBytes": { "type": "integer", "minimum": 0 },
                        "minSizeSavingsPercent": { "type": "number" },
                        "jobs": { "type": ["integer", "null"], "minimum": 1 },
                        "noBackup": { "type": "boolean" }
                    },
                    "required": [
                        "apply",
                        "estimateSize",
                        "minSizeSavingsBytes",
                        "minSizeSavingsPercent",
                        "jobs",
                        "noBackup"
                    ],
                    "additionalProperties": false
                },
                "summary": {
                    "type": "object",
                    "properties": {
                        "total": { "type": "integer", "minimum": 0 },
                        "changed": { "type": "integer", "minimum": 0 },
                        "unchanged": { "type": "integer", "minimum": 0 },
                        "skipped": { "type": "integer", "minimum": 0 },
                        "applied": { "type": "integer", "minimum": 0 },
                        "failed": { "type": "integer", "minimum": 0 },
                        "signedTotal": { "type": "integer", "minimum": 0 },
                        "signedSkipped": { "type": "integer", "minimum": 0 },
                        "passwordProtectedSkipped": { "type": "integer", "minimum": 0 },
                        "optimizationChecked": { "type": "integer", "minimum": 0 },
                        "optimizationRecommended": { "type": "integer", "minimum": 0 },
                        "estimatedSavedTotalBytes": { "type": "integer" },
                        "actualSavedTotalBytes": { "type": "integer" }
                    },
                    "required": [
                        "total",
                        "changed",
                        "unchanged",
                        "skipped",
                        "applied",
                        "failed",
                        "signedTotal",
                        "signedSkipped",
                        "passwordProtectedSkipped",
                        "optimizationChecked",
                        "optimizationRecommended",
                        "estimatedSavedTotalBytes",
                        "actualSavedTotalBytes"
                    ],
                    "additionalProperties": false
                },
                "backupRoot": { "type": "string" },
                "reportPath": { "type": "string" },
                "files": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "sizeBytes": { "type": "integer", "minimum": 0 },
                            "skipped": { "type": "boolean" },
                            "skipReason": { "type": "string" },
                            "changed": { "type": "boolean" },
                            "plannedActions": { "type": "array", "items": { "type": "string" } },
                            "titleBefore": { "type": "string" },
                            "titleAfter": { "type": "string" },
                            "fieldsToStrip": { "type": "array", "items": { "type": "string" } },
                            "xmpPresent": { "type": "boolean" },
                            "versionBefore": { "type": "string" },
                            "versionAfter": { "type": "string" },
                            "signed": { "type": "boolean" },
                            "passwordProtected": { "type": "boolean" },
                            "optimizationChecked": { "type": "boolean" },
                            "optimizationRecommended": { "type": "boolean" },
                            "optimizationError": { "type": "string" },
                            "estimatedSizeAfterBytes": { "type": ["integer", "null"] },
                            "estimatedSavedBytes": { "type": ["integer", "null"] },
                            "estimatedSavedPercent": { "type": ["number", "null"] },
                            "applied": { "type": "boolean" },
                            "applyError": { "type": "string" },
                            "applyNote": { "type": "string" },
                            "sizeAfterBytes": { "type": ["integer", "null"] },
                            "actualSavedBytes": { "type": ["integer", "null"] },
                            "actualSavedPercent": { "type": ["number", "null"] }
                        },
                        "required": [
                            "path",
                            "sizeBytes",
                            "skipped",
                            "skipReason",
                            "changed",
                            "plannedActions",
                            "titleBefore",
                            "titleAfter",
                            "fieldsToStrip",
                            "xmpPresent",
                            "versionBefore",
                            "versionAfter",
                            "signed",
                            "passwordProtected",
                            "optimizationChecked",
                            "optimizationRecommended",
                            "optimizationError",
                            "estimatedSizeAfterBytes",
                            "estimatedSavedBytes",
                            "estimatedSavedPercent",
                            "applied",
                            "applyError",
                            "applyNote",
                            "sizeAfterBytes",
                            "actualSavedBytes",
                            "actualSavedPercent"
                        ],
                        "additionalProperties": false
                    }
                }
            },
            "required": [
                "timestamp",
                "mode",
                "targetPath",
                "options",
                "summary",
                "backupRoot",
                "reportPath",
                "files"
            ],
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "apply": { "type": "boolean" },
                "estimateSize": { "type": "boolean" },
                "minSizeSavingsBytes": { "type": "integer", "minimum": 0, "default": 1024 },
                "minSizeSavingsPercent": { "type": "number", "default": 0.5 },
                "jobs": { "type": "integer", "minimum": 1 },
                "noBackup": { "type": "boolean" }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        idempotent: false,
        rate_limit: None,
        example: ToolExample {
            command: "pdf optimize ./docs --estimate-size",
            description: "Plan a directory run and include qpdf-backed size estimates in the report.",
        },
    }
}

fn global_flag_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "type": { "type": "string" },
            "description": { "type": "string" },
            "default": {}
        },
        "required": ["name", "type", "description", "default"],
        "additionalProperties": false
    })
}

fn tool_descriptor_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "command": { "type": "string" },
            "category": { "type": "string" },
            "description": { "type": "string" },
            "parameters": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "type": { "type": "string" },
                        "required": { "type": "boolean" },
                        "description": { "type": "string" }
                    },
                    "required": ["name", "type", "required", "description"],
                    "additionalProperties": false
                }
            },
            "outputFields": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "type": { "type": "string" },
                        "description": { "type": "string" }
                    },
                    "required": ["name", "type", "description"],
                    "additionalProperties": false
                }
            },
            "outputSchema": {},
            "inputSchema": {},
            "idempotent": { "type": "boolean" },
            "rateLimit": { "type": ["string", "null"] },
            "example": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "description": { "type": "string" }
                },
                "required": ["command", "description"],
                "additionalProperties": false
            }
        },
        "required": [
            "name",
            "command",
            "category",
            "description",
            "parameters",
            "outputFields",
            "outputSchema",
            "inputSchema",
            "idempotent",
            "rateLimit",
            "example"
        ],
        "additionalProperties": false
    })
}
