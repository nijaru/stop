use thiserror::Error;

/// Errors that can occur during filter parsing or evaluation.
#[derive(Debug, Error)]
pub enum FilterError {
    #[error("Invalid filter expression: {0}")]
    InvalidExpression(String),

    #[error("Unknown field '{0}'. Valid fields: cpu, mem, pid, name, user")]
    UnknownField(String),

    #[error("Unknown operator '{0}'. Valid operators: >, >=, <, <=, ==, !=")]
    UnknownOperator(String),

    #[error("Invalid value '{value}' for field '{field}': {reason}")]
    InvalidValue {
        field: String,
        value: String,
        reason: String,
    },

    #[error("Type mismatch: operator '{op}' cannot be used with field '{field}'")]
    TypeMismatch { op: String, field: String },
}

/// Comparison operators for filter expressions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterOp {
    /// Greater than (>)
    Gt,
    /// Greater than or equal (>=)
    Gte,
    /// Less than (<)
    Lt,
    /// Less than or equal (<=)
    Lte,
    /// Equal (==)
    Eq,
    /// Not equal (!=)
    Ne,
}

impl FilterOp {
    fn from_str(s: &str) -> Result<Self, FilterError> {
        match s {
            ">" => Ok(Self::Gt),
            ">=" => Ok(Self::Gte),
            "<" => Ok(Self::Lt),
            "<=" => Ok(Self::Lte),
            "==" => Ok(Self::Eq),
            "!=" => Ok(Self::Ne),
            _ => Err(FilterError::UnknownOperator(s.to_string())),
        }
    }

    fn is_comparison(&self) -> bool {
        matches!(self, Self::Gt | Self::Gte | Self::Lt | Self::Lte)
    }
}

/// Fields that can be filtered on in process queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterField {
    /// CPU usage percentage (numeric)
    Cpu,
    /// Memory usage percentage (numeric)
    Mem,
    /// Process ID (numeric)
    Pid,
    /// Process name (string, case-insensitive)
    Name,
    /// User ID or name (string, case-sensitive)
    User,
}

impl FilterField {
    fn from_str(s: &str) -> Result<Self, FilterError> {
        match s.to_lowercase().as_str() {
            "cpu" => Ok(Self::Cpu),
            "mem" | "memory" => Ok(Self::Mem),
            "pid" => Ok(Self::Pid),
            "name" => Ok(Self::Name),
            "user" => Ok(Self::User),
            _ => Err(FilterError::UnknownField(s.to_string())),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Self::Cpu | Self::Mem | Self::Pid)
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Mem => "mem",
            Self::Pid => "pid",
            Self::Name => "name",
            Self::User => "user",
        }
    }
}

/// Values that can be compared in filter expressions.
///
/// Stores both original and lowercase versions of strings for efficient matching.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
    /// Floating-point value (for cpu, mem fields)
    Float(f32),
    /// Integer value (for pid field)
    Int(u32),
    /// String value with pre-computed lowercase for case-insensitive matching
    String { original: String, lowercase: String },
}

/// A single filter condition (field operator value).
///
/// Example: `cpu > 10`, `name == chrome`
#[derive(Debug, Clone)]
pub struct Filter {
    field: FilterField,
    op: FilterOp,
    value: FilterValue,
}

/// Filter expression tree supporting AND/OR logic.
///
/// Parses expressions like:
/// - Simple: `cpu > 10`
/// - AND: `cpu > 10 and mem > 5`
/// - OR: `cpu > 50 or name == chrome`
/// - Mixed: `cpu > 50 or mem > 10 and pid < 1000` (OR has lower precedence)
#[derive(Debug, Clone)]
pub enum FilterExpr {
    /// Single filter condition
    Simple(Filter),
    /// Logical AND (both conditions must match)
    And(Box<FilterExpr>, Box<FilterExpr>),
    /// Logical OR (at least one condition must match)
    Or(Box<FilterExpr>, Box<FilterExpr>),
}

fn find_keyword(s: &str, keyword: &str) -> Option<usize> {
    let keyword_lower = keyword.to_lowercase();
    let s_lower = s.to_lowercase();

    let mut pos = 0;
    while let Some(found) = s_lower[pos..].find(&keyword_lower) {
        let actual_pos = pos + found;

        // Check if it's a whole word (surrounded by spaces or boundaries)
        let before_ok = actual_pos == 0
            || s_lower
                .chars()
                .nth(actual_pos - 1)
                .is_none_or(|c| c.is_whitespace());
        let after_pos = actual_pos + keyword_lower.len();
        let after_ok = after_pos >= s_lower.len()
            || s_lower
                .chars()
                .nth(after_pos)
                .is_none_or(|c| c.is_whitespace());

        if before_ok && after_ok {
            return Some(actual_pos);
        }

        pos = actual_pos + 1;
    }
    None
}

impl FilterExpr {
    /// Parses a filter expression string into a FilterExpr tree.
    ///
    /// Supports AND/OR logic with proper precedence (OR is lower precedence than AND).
    /// Keywords (and, or) are case-insensitive.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let expr = FilterExpr::parse("cpu > 10")?;
    /// let expr = FilterExpr::parse("cpu > 10 and mem > 5")?;
    /// let expr = FilterExpr::parse("cpu > 50 or name == chrome")?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `FilterError` if the expression is invalid, contains unknown fields/operators,
    /// or has type mismatches (e.g., using > with string fields).
    pub fn parse(expression: &str) -> Result<Self, FilterError> {
        let expr = expression.trim();

        // Split on OR (lowest precedence)
        if let Some(pos) = find_keyword(expr, "or") {
            let left_str = expr[..pos].trim();
            let right_str = expr[pos + 2..].trim();

            let left = Self::parse(left_str)?;
            let right = Self::parse(right_str)?;

            return Ok(FilterExpr::Or(Box::new(left), Box::new(right)));
        }

        // Split on AND (higher precedence)
        if let Some(pos) = find_keyword(expr, "and") {
            let left_str = expr[..pos].trim();
            let right_str = expr[pos + 3..].trim();

            let left = Self::parse(left_str)?;
            let right = Self::parse(right_str)?;

            return Ok(FilterExpr::And(Box::new(left), Box::new(right)));
        }

        // Simple condition
        Filter::parse_simple(expr).map(FilterExpr::Simple)
    }

    /// Tests whether a process matches this filter expression.
    ///
    /// # Arguments
    ///
    /// * `process` - The process to test against this filter
    ///
    /// # Returns
    ///
    /// `true` if the process matches the filter expression, `false` otherwise.
    pub fn matches(&self, process: &crate::ProcessInfo) -> bool {
        match self {
            FilterExpr::Simple(f) => f.matches(process),
            FilterExpr::And(l, r) => l.matches(process) && r.matches(process),
            FilterExpr::Or(l, r) => l.matches(process) || r.matches(process),
        }
    }
}

impl Filter {
    fn parse_simple(expression: &str) -> Result<Self, FilterError> {
        let expr = expression.trim();

        if expr.is_empty() {
            return Err(FilterError::InvalidExpression(
                "Empty filter expression".to_string(),
            ));
        }

        // Try to find operator (greedy match: >= before >)
        let operators = [">=", "<=", "!=", "==", ">", "<"];
        let mut found_op: Option<(&str, FilterOp, usize)> = None;

        for op_str in &operators {
            if let Some(pos) = expr.find(op_str)
                && let Ok(op) = FilterOp::from_str(op_str)
            {
                found_op = Some((op_str, op, pos));
                break;
            }
        }

        let (op_str, op, op_pos) = found_op.ok_or_else(|| {
            FilterError::InvalidExpression(
                "No valid operator found. Use: >, >=, <, <=, ==, !=".to_string(),
            )
        })?;

        let field_str = expr[..op_pos].trim();
        let value_str = expr[op_pos + op_str.len()..].trim();

        if field_str.is_empty() {
            return Err(FilterError::InvalidExpression(
                "Missing field before operator".to_string(),
            ));
        }

        if value_str.is_empty() {
            return Err(FilterError::InvalidExpression(
                "Missing value after operator".to_string(),
            ));
        }

        let field = FilterField::from_str(field_str)?;

        // Validate operator compatibility with field
        if op.is_comparison() && !field.is_numeric() {
            return Err(FilterError::TypeMismatch {
                op: op_str.to_string(),
                field: field.name().to_string(),
            });
        }

        // Parse value based on field type
        let value = match field {
            FilterField::Cpu | FilterField::Mem => value_str
                .parse::<f32>()
                .map(FilterValue::Float)
                .map_err(|_| FilterError::InvalidValue {
                    field: field.name().to_string(),
                    value: value_str.to_string(),
                    reason: "Expected a number (e.g., 10 or 5.5)".to_string(),
                })?,
            FilterField::Pid => value_str
                .parse::<u32>()
                .map(FilterValue::Int)
                .map_err(|_| FilterError::InvalidValue {
                    field: field.name().to_string(),
                    value: value_str.to_string(),
                    reason: "Expected an integer (e.g., 1000)".to_string(),
                })?,
            FilterField::Name | FilterField::User => {
                let original = value_str.to_string();
                let lowercase = original.to_lowercase();
                FilterValue::String {
                    original,
                    lowercase,
                }
            }
        };

        Ok(Self { field, op, value })
    }

    /// Tests whether a process matches this filter condition.
    ///
    /// # Arguments
    ///
    /// * `process` - The process to test against this filter
    ///
    /// # Returns
    ///
    /// `true` if the process matches the filter condition, `false` otherwise.
    pub fn matches(&self, process: &crate::ProcessInfo) -> bool {
        match (&self.field, &self.value, &self.op) {
            // CPU comparisons
            (FilterField::Cpu, FilterValue::Float(val), op) => {
                Self::compare_float(process.cpu_percent, *val, *op)
            }
            // Memory comparisons
            (FilterField::Mem, FilterValue::Float(val), op) => {
                Self::compare_float(process.memory_percent, *val, *op)
            }
            // PID comparisons
            (FilterField::Pid, FilterValue::Int(val), op) => {
                Self::compare_int(process.pid, *val, *op)
            }
            // Name matching (case-insensitive contains for ==, inverse for !=)
            (FilterField::Name, FilterValue::String { lowercase, .. }, FilterOp::Eq) => {
                process.name.to_lowercase().contains(lowercase)
            }
            (FilterField::Name, FilterValue::String { lowercase, .. }, FilterOp::Ne) => {
                !process.name.to_lowercase().contains(lowercase)
            }
            // User matching (exact match, case-sensitive)
            (FilterField::User, FilterValue::String { original, .. }, FilterOp::Eq) => {
                &process.user == original
            }
            (FilterField::User, FilterValue::String { original, .. }, FilterOp::Ne) => {
                &process.user != original
            }
            // Invalid combinations (should be caught during parsing)
            _ => false,
        }
    }

    fn compare_float(a: f32, b: f32, op: FilterOp) -> bool {
        match op {
            FilterOp::Gt => a > b,
            FilterOp::Gte => a >= b,
            FilterOp::Lt => a < b,
            FilterOp::Lte => a <= b,
            FilterOp::Eq => (a - b).abs() < f32::EPSILON,
            FilterOp::Ne => (a - b).abs() >= f32::EPSILON,
        }
    }

    fn compare_int(a: u32, b: u32, op: FilterOp) -> bool {
        match op {
            FilterOp::Gt => a > b,
            FilterOp::Gte => a >= b,
            FilterOp::Lt => a < b,
            FilterOp::Lte => a <= b,
            FilterOp::Eq => a == b,
            FilterOp::Ne => a != b,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_filter() {
        let expr = FilterExpr::parse("cpu > 10").unwrap();
        if let FilterExpr::Simple(filter) = expr {
            assert!(matches!(filter.field, FilterField::Cpu));
            assert!(matches!(filter.op, FilterOp::Gt));
            assert!(
                matches!(filter.value, FilterValue::Float(v) if (v - 10.0).abs() < f32::EPSILON)
            );
        } else {
            panic!("Expected FilterExpr::Simple");
        }
    }

    #[test]
    fn test_parse_mem_filter() {
        let expr = FilterExpr::parse("mem >= 5.5").unwrap();
        if let FilterExpr::Simple(filter) = expr {
            assert!(matches!(filter.field, FilterField::Mem));
            assert!(matches!(filter.op, FilterOp::Gte));
        } else {
            panic!("Expected FilterExpr::Simple");
        }
    }

    #[test]
    fn test_parse_name_filter() {
        let expr = FilterExpr::parse("name == chrome").unwrap();
        if let FilterExpr::Simple(filter) = expr {
            assert!(matches!(filter.field, FilterField::Name));
            assert!(matches!(filter.op, FilterOp::Eq));
            assert!(matches!(filter.value, FilterValue::String { .. }));
        } else {
            panic!("Expected FilterExpr::Simple");
        }
    }

    #[test]
    fn test_invalid_field() {
        let result = FilterExpr::parse("invalid > 10");
        assert!(matches!(result, Err(FilterError::UnknownField(_))));
    }

    #[test]
    fn test_invalid_operator() {
        // "cpu >> 10" will parse ">" first, leaving "> 10" as value
        // This results in InvalidValue, not InvalidExpression
        let result = FilterExpr::parse("cpu >> 10");
        assert!(matches!(result, Err(FilterError::InvalidValue { .. })));
    }

    #[test]
    fn test_type_mismatch() {
        let result = FilterExpr::parse("name > 10");
        assert!(matches!(result, Err(FilterError::TypeMismatch { .. })));
    }

    #[test]
    fn test_invalid_value() {
        let result = FilterExpr::parse("cpu > abc");
        assert!(matches!(result, Err(FilterError::InvalidValue { .. })));
    }

    #[test]
    fn test_empty_expression() {
        let result = FilterExpr::parse("");
        assert!(matches!(result, Err(FilterError::InvalidExpression(_))));
    }

    // Compound expression tests
    #[test]
    fn test_and_filter() {
        let expr = FilterExpr::parse("cpu > 10 and mem > 5").unwrap();

        // Test process that matches both conditions
        let matching_process = crate::ProcessInfo {
            pid: 1,
            name: "test".to_string(),
            cpu_percent: 15.0,
            memory_bytes: 1024,
            memory_percent: 10.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&matching_process));

        // Test process that matches only first condition
        let partial_match_1 = crate::ProcessInfo {
            pid: 2,
            name: "test".to_string(),
            cpu_percent: 15.0,
            memory_bytes: 1024,
            memory_percent: 3.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(!expr.matches(&partial_match_1));

        // Test process that matches only second condition
        let partial_match_2 = crate::ProcessInfo {
            pid: 3,
            name: "test".to_string(),
            cpu_percent: 5.0,
            memory_bytes: 1024,
            memory_percent: 10.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(!expr.matches(&partial_match_2));
    }

    #[test]
    fn test_or_filter() {
        let expr = FilterExpr::parse("cpu > 50 or mem > 10").unwrap();

        // Test process that matches first condition
        let match_cpu = crate::ProcessInfo {
            pid: 1,
            name: "test".to_string(),
            cpu_percent: 60.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&match_cpu));

        // Test process that matches second condition
        let match_mem = crate::ProcessInfo {
            pid: 2,
            name: "test".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 15.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&match_mem));

        // Test process that matches both conditions
        let match_both = crate::ProcessInfo {
            pid: 3,
            name: "test".to_string(),
            cpu_percent: 60.0,
            memory_bytes: 1024,
            memory_percent: 15.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&match_both));

        // Test process that matches neither condition
        let match_none = crate::ProcessInfo {
            pid: 4,
            name: "test".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(!expr.matches(&match_none));
    }

    #[test]
    fn test_case_insensitive_keywords() {
        assert!(FilterExpr::parse("cpu > 10 AND mem > 5").is_ok());
        assert!(FilterExpr::parse("cpu > 10 And mem > 5").is_ok());
        assert!(FilterExpr::parse("cpu > 10 and mem > 5").is_ok());

        assert!(FilterExpr::parse("cpu > 10 OR mem > 5").is_ok());
        assert!(FilterExpr::parse("cpu > 10 Or mem > 5").is_ok());
        assert!(FilterExpr::parse("cpu > 10 or mem > 5").is_ok());
    }

    #[test]
    fn test_mixed_and_or_precedence() {
        // Test: a OR b AND c should parse as: a OR (b AND c)
        let expr = FilterExpr::parse("cpu > 50 or mem > 10 and pid < 1000").unwrap();

        // Process with cpu > 50 should match (first condition of OR)
        let match_cpu = crate::ProcessInfo {
            pid: 5000,
            name: "test".to_string(),
            cpu_percent: 60.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&match_cpu));

        // Process with mem > 10 AND pid < 1000 should match (second part)
        let match_and = crate::ProcessInfo {
            pid: 500,
            name: "test".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 15.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&match_and));

        // Process with only mem > 10 but pid >= 1000 should NOT match
        let no_match = crate::ProcessInfo {
            pid: 5000,
            name: "test".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 15.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(!expr.matches(&no_match));
    }

    #[test]
    fn test_keyword_in_string_values() {
        // "android" contains "and" but should not be parsed as keyword
        let expr = FilterExpr::parse("name == android").unwrap();

        let process = crate::ProcessInfo {
            pid: 1,
            name: "android_app".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&process));
    }

    #[test]
    fn test_multiple_spaces_in_compound() {
        // Should handle extra whitespace gracefully
        assert!(FilterExpr::parse("cpu > 10   and   mem > 5").is_ok());
        assert!(FilterExpr::parse("cpu > 10 or  mem > 5").is_ok());
    }

    #[test]
    fn test_empty_condition_in_compound() {
        // Should fail gracefully with empty conditions
        let result = FilterExpr::parse("cpu > 10 and");
        assert!(result.is_err());

        let result = FilterExpr::parse("or mem > 5");
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_string_filters_with_and_or() {
        // Chrome OR Firefox
        let expr = FilterExpr::parse("name == chrome or name == firefox").unwrap();

        let chrome = crate::ProcessInfo {
            pid: 1,
            name: "chrome".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&chrome));

        let firefox = crate::ProcessInfo {
            pid: 2,
            name: "firefox".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(expr.matches(&firefox));

        let other = crate::ProcessInfo {
            pid: 3,
            name: "safari".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 1024,
            memory_percent: 5.0,
            user: "user".to_string(),
            command: "cmd".to_string(),
            thread_count: 1,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            open_files: None,
        };
        assert!(!expr.matches(&other));
    }
}
