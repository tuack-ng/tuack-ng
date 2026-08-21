use serde_json::{Value, json};

/// 三份 conf.json 的 JSON Schema（draft-07，键名与 FileView/文件一致）
pub fn schema() -> Value {
    json!({
        "contest": {
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "contest conf.json",
            "type": "object",
            "required": ["version", "folder", "name", "subdir", "title", "short title"],
            "properties": {
                "version": { "type": "integer", "minimum": 3 },
                "folder": { "const": "contest" },
                "name": { "type": "string" },
                "subdir": { "type": "array", "items": { "type": "string" } },
                "title": { "type": "string" },
                "short title": { "type": "string" },
                "use_pretest": { "type": "boolean" },
                "noi_style": { "type": "boolean" },
                "file_io": { "type": "boolean" }
            }
        },
        "day": {
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "day conf.json",
            "type": "object",
            "required": ["version", "folder", "name", "subdir", "title", "compile"],
            "properties": {
                "version": { "type": "integer", "minimum": 3 },
                "folder": { "const": "day" },
                "name": { "type": "string" },
                "subdir": { "type": "array", "items": { "type": "string" } },
                "title": { "type": "string" },
                "compile": { "type": "object", "additionalProperties": { "type": "string" } },
                "start time": { "type": "array", "items": { "type": "integer" }, "minItems": 6, "maxItems": 6 },
                "end time": { "type": "array", "items": { "type": "integer" }, "minItems": 6, "maxItems": 6 },
                "use_pretest": { "type": "boolean" },
                "noi_style": { "type": "boolean" },
                "file_io": { "type": "boolean" }
            }
        },
        "problem": {
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "problem conf.json",
            "type": "object",
            "required": ["version", "folder", "type", "name", "title", "time limit", "memory limit", "dmk"],
            "$defs": {
                "checker": {
                    "type": "object",
                    "required": ["source"],
                    "properties": {
                        "source": { "type": "string" },
                        "deps": { "type": "array", "items": { "type": "string" } }
                    }
                },
                "gen": {
                    "type": "object",
                    "required": ["gen"],
                    "properties": {
                        "gen": { "type": "string" },
                        "deps": { "type": "array", "items": { "type": "string" } },
                        "validate": { "type": "boolean" }
                    }
                },
                "args": {
                    "type": "object",
                    "additionalProperties": {
                        "oneOf": [
                            { "type": "integer" },
                            { "type": "number" },
                            { "type": "string" },
                            { "type": "boolean" }
                        ]
                    }
                },
                "sampleItem": {
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "integer", "minimum": 1 },
                        "input": { "type": "string" },
                        "output": { "type": "string" },
                        "args": { "$ref": "#/$defs/args" },
                        "dmk": { "enum": ["skip", "input", "output", "on"] }
                    }
                },
                "dataItemSingle": {
                    "type": "object",
                    "required": ["id", "score"],
                    "properties": {
                        "id": { "type": "integer", "minimum": 1 },
                        "score": { "type": "integer", "minimum": 0 },
                        "subtask": { "type": "integer", "minimum": 0 },
                        "input": { "type": "string" },
                        "output": { "type": "string" },
                        "args": { "$ref": "#/$defs/args" },
                        "dmk": { "enum": ["skip", "input", "output", "on"] }
                    }
                },
                "dataItemBundle": {
                    "type": "object",
                    "required": ["id", "score"],
                    "properties": {
                        "id": { "type": "array", "items": { "type": "integer", "minimum": 1 } },
                        "score": { "type": "integer", "minimum": 0 },
                        "subtask": { "type": "integer", "minimum": 0 },
                        "args": { "$ref": "#/$defs/args" },
                        "dmk": { "enum": ["skip", "input", "output", "on"] }
                    }
                }
            },
            "properties": {
                "version": { "type": "integer", "minimum": 3 },
                "folder": { "const": "problem" },
                "type": { "enum": ["program", "output", "interactive"] },
                "name": { "type": "string" },
                "title": { "type": "string" },
                "time limit": { "type": "number", "exclusiveMinimum": 0 },
                "memory limit": { "type": "string" },
                "dmk": { "enum": ["skip", "input", "output", "on"] },
                "args": { "$ref": "#/$defs/args" },
                "interactive": {
                    "type": "object",
                    "required": ["grader", "header"],
                    "properties": {
                        "grader": { "type": "string" },
                        "header": { "type": "string" },
                        "sample_grader": { "type": "string" },
                        "dmk_grader": { "type": "string" }
                    }
                },
                "generator": {
                    "type": "object",
                    "required": ["data"],
                    "properties": {
                        "data": { "$ref": "#/$defs/gen" },
                        "sample": { "$ref": "#/$defs/gen" }
                    }
                },
                "checker": {
                    "type": "object",
                    "required": ["data"],
                    "properties": {
                        "data": { "$ref": "#/$defs/checker" },
                        "sample": { "$ref": "#/$defs/checker" }
                    }
                },
                "validator": {
                    "type": "object",
                    "required": ["data"],
                    "properties": {
                        "data": { "$ref": "#/$defs/checker" },
                        "sample": { "$ref": "#/$defs/checker" }
                    }
                },
                "samples": { "type": "array", "items": { "$ref": "#/$defs/sampleItem" } },
                "data": {
                    "type": "array",
                    "items": {
                        "oneOf": [
                            { "$ref": "#/$defs/dataItemSingle" },
                            { "$ref": "#/$defs/dataItemBundle" }
                        ]
                    }
                },
                "subtasks": {
                    "type": "object",
                    "additionalProperties": { "enum": ["sum", "max", "min"] }
                },
                "tests": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "object",
                        "required": ["expected", "path"],
                        "properties": {
                            "expected": {
                                "oneOf": [
                                    { "type": "string" },
                                    { "type": "array", "items": { "type": "string" } }
                                ]
                            },
                            "path": { "type": "string" }
                        }
                    }
                }
            }
        }
    })
}
