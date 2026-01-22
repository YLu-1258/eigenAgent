// src-tauri/src/tools/implementations/calculator.rs

use meval;

use crate::tools::types::{ToolCallRequest, ToolCallResult};

pub fn execute(request: &ToolCallRequest) -> ToolCallResult {
    let expression = match request.arguments.get("expression").and_then(|v| v.as_str()) {
        Some(expr) => expr,
        None => {
            return ToolCallResult::error(
                request.call_id.clone(),
                "Missing required parameter: expression".to_string(),
            )
        }
    };

    // Clean up the expression
    let cleaned = expression
        .trim()
        .replace("×", "*")
        .replace("÷", "/")
        .replace("^", ".powf");

    match meval::eval_str(&cleaned) {
        Ok(result) => {
            let output = if result.fract() == 0.0 && result.abs() < 1e15 {
                // Display as integer if it's a whole number
                format!("{} = {}", expression, result as i64)
            } else {
                // Display with reasonable precision
                format!("{} = {}", expression, result)
            };
            ToolCallResult::success(request.call_id.clone(), output)
        }
        Err(e) => ToolCallResult::error(
            request.call_id.clone(),
            format!("Failed to evaluate expression '{}': {}", expression, e),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_request(expression: &str) -> ToolCallRequest {
        ToolCallRequest {
            tool_id: "calculator".to_string(),
            call_id: "test".to_string(),
            arguments: json!({ "expression": expression }),
        }
    }

    #[test]
    fn test_basic_arithmetic() {
        let result = execute(&make_request("2 + 2"));
        assert!(result.success);
        assert!(result.output.contains("4"));
    }

    #[test]
    fn test_multiplication() {
        let result = execute(&make_request("3 * 4"));
        assert!(result.success);
        assert!(result.output.contains("12"));
    }

    #[test]
    fn test_division() {
        let result = execute(&make_request("10 / 2"));
        assert!(result.success);
        assert!(result.output.contains("5"));
    }

    #[test]
    fn test_sqrt() {
        let result = execute(&make_request("sqrt(16)"));
        assert!(result.success);
        assert!(result.output.contains("4"));
    }

    #[test]
    fn test_complex_expression() {
        let result = execute(&make_request("(2 + 3) * 4 - 1"));
        assert!(result.success);
        assert!(result.output.contains("19"));
    }

    #[test]
    fn test_invalid_expression() {
        let result = execute(&make_request("2 + + 2"));
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
