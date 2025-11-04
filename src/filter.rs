use thiserror::Error;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterField {
    Cpu,
    Mem,
    Pid,
    Name,
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

#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
    Float(f32),
    Int(u32),
    String(String),
}

#[derive(Debug, Clone)]
pub struct Filter {
    field: FilterField,
    op: FilterOp,
    value: FilterValue,
}

impl Filter {
    pub fn parse(expression: &str) -> Result<Self, FilterError> {
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
            FilterField::Name | FilterField::User => FilterValue::String(value_str.to_string()),
        };

        Ok(Self { field, op, value })
    }

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
            (FilterField::Name, FilterValue::String(val), FilterOp::Eq) => {
                process.name.to_lowercase().contains(&val.to_lowercase())
            }
            (FilterField::Name, FilterValue::String(val), FilterOp::Ne) => {
                !process.name.to_lowercase().contains(&val.to_lowercase())
            }
            // User matching (exact match, case-sensitive)
            (FilterField::User, FilterValue::String(val), FilterOp::Eq) => &process.user == val,
            (FilterField::User, FilterValue::String(val), FilterOp::Ne) => &process.user != val,
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
        let filter = Filter::parse("cpu > 10").unwrap();
        assert!(matches!(filter.field, FilterField::Cpu));
        assert!(matches!(filter.op, FilterOp::Gt));
        assert!(matches!(filter.value, FilterValue::Float(v) if (v - 10.0).abs() < f32::EPSILON));
    }

    #[test]
    fn test_parse_mem_filter() {
        let filter = Filter::parse("mem >= 5.5").unwrap();
        assert!(matches!(filter.field, FilterField::Mem));
        assert!(matches!(filter.op, FilterOp::Gte));
    }

    #[test]
    fn test_parse_name_filter() {
        let filter = Filter::parse("name == chrome").unwrap();
        assert!(matches!(filter.field, FilterField::Name));
        assert!(matches!(filter.op, FilterOp::Eq));
        assert!(matches!(filter.value, FilterValue::String(_)));
    }

    #[test]
    fn test_invalid_field() {
        let result = Filter::parse("invalid > 10");
        assert!(matches!(result, Err(FilterError::UnknownField(_))));
    }

    #[test]
    fn test_invalid_operator() {
        // "cpu >> 10" will parse ">" first, leaving "> 10" as value
        // This results in InvalidValue, not InvalidExpression
        let result = Filter::parse("cpu >> 10");
        assert!(matches!(result, Err(FilterError::InvalidValue { .. })));
    }

    #[test]
    fn test_type_mismatch() {
        let result = Filter::parse("name > 10");
        assert!(matches!(result, Err(FilterError::TypeMismatch { .. })));
    }

    #[test]
    fn test_invalid_value() {
        let result = Filter::parse("cpu > abc");
        assert!(matches!(result, Err(FilterError::InvalidValue { .. })));
    }

    #[test]
    fn test_empty_expression() {
        let result = Filter::parse("");
        assert!(matches!(result, Err(FilterError::InvalidExpression(_))));
    }
}
