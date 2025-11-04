# Compound Filter Expressions Research (2025-11-04)

## Goal
Extend filter syntax to support multiple conditions with AND/OR logic.

## Design Considerations

### Syntax Options

**Option 1: Shell-style (&&, ||)**
```bash
stop --filter "cpu > 10 && mem > 5"
stop --filter "name == chrome || name == firefox"
```
- Pro: Familiar to shell users
- Con: Requires shell escaping, verbose

**Option 2: SQL-style (AND, OR)**
```bash
stop --filter "cpu > 10 AND mem > 5"
stop --filter "name == chrome OR name == firefox"
```
- Pro: Clear, readable, no escaping needed
- Con: Case sensitivity question (AND vs and)

**Option 3: Comma/pipe shorthand**
```bash
stop --filter "cpu > 10, mem > 5"      # AND
stop --filter "name == chrome | firefox"  # OR
```
- Pro: Concise
- Con: Ambiguous, limits future extensions

**Decision: Option 2 (SQL-style) with case-insensitive keywords**
- Most explicit and readable
- Easy for AI agents to construct
- No shell escaping issues
- Aligns with jq's `and`/`or` but case-insensitive for UX

### Precedence Rules

Standard boolean precedence: AND before OR
- `a OR b AND c` → `a OR (b AND c)`
- Use parentheses for explicit grouping (Phase 3)

**Phase 2 (current): No parentheses, just AND/OR**
- Keep it simple: left-to-right evaluation
- Users can chain multiple --filter flags for complex queries

### Implementation Strategy

**Two-phase approach:**

**Phase 2a (current):** Simple AND/OR chains
- Parse: Split on "and"/"or" (case-insensitive)
- Evaluate: Left-to-right with short-circuit
- Example: `cpu > 10 and mem > 5 or name == chrome`

**Phase 3 (future):** Parentheses support
- Full expression parser (recursive descent)
- Proper precedence handling
- Example: `(cpu > 10 and mem > 5) or (name == chrome and user == root)`

## Parser Design

```rust
enum FilterExpr {
    Simple(Filter),           // Single condition
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
}

impl FilterExpr {
    fn parse(s: &str) -> Result<Self, FilterError> {
        // Split on OR (lowest precedence)
        if let Some(pos) = find_keyword(s, "or") {
            let left = Self::parse(&s[..pos])?;
            let right = Self::parse(&s[pos+2..])?;
            return Ok(FilterExpr::Or(Box::new(left), Box::new(right)));
        }

        // Split on AND (higher precedence)
        if let Some(pos) = find_keyword(s, "and") {
            let left = Self::parse(&s[..pos])?;
            let right = Self::parse(&s[pos+3..])?;
            return Ok(FilterExpr::And(Box::new(left), Box::new(right)));
        }

        // Simple condition
        Filter::parse(s).map(FilterExpr::Simple)
    }

    fn matches(&self, process: &ProcessInfo) -> bool {
        match self {
            FilterExpr::Simple(f) => f.matches(process),
            FilterExpr::And(l, r) => l.matches(process) && r.matches(process),
            FilterExpr::Or(l, r) => l.matches(process) || r.matches(process),
        }
    }
}
```

## Edge Cases

1. **Keyword in string values**: `name == android` (won't match "and" inside)
   - Solution: Only split on whole-word boundaries with spaces

2. **Multiple spaces**: `cpu > 10   and   mem > 5`
   - Solution: Trim after split

3. **Case variations**: `AND`, `And`, `and`
   - Solution: Case-insensitive keyword matching

4. **Empty conditions**: `cpu > 10 and`
   - Solution: Return parse error

## Examples

```bash
# High CPU AND high memory
stop --filter "cpu > 50 and mem > 10"

# Chrome OR Firefox processes
stop --filter "name == chrome or name == firefox"

# Complex: High resource usage
stop --filter "cpu > 50 or mem > 10"

# System processes with high CPU
stop --filter "pid < 1000 and cpu > 5"

# NOT operator (future, use != for now)
stop --filter "cpu > 10 and name != chrome"
```

## Testing Strategy

```rust
#[test]
fn test_and_filter() {
    let expr = FilterExpr::parse("cpu > 10 and mem > 5").unwrap();
    // Test both conditions must match
}

#[test]
fn test_or_filter() {
    let expr = FilterExpr::parse("cpu > 10 or mem > 5").unwrap();
    // Test either condition matches
}

#[test]
fn test_case_insensitive_keywords() {
    assert!(FilterExpr::parse("cpu > 10 AND mem > 5").is_ok());
    assert!(FilterExpr::parse("cpu > 10 Or mem > 5").is_ok());
}

#[test]
fn test_mixed_and_or() {
    // Left-to-right evaluation in Phase 2
    let expr = FilterExpr::parse("cpu > 10 and mem > 5 or pid < 1000").unwrap();
}
```

## Migration Path

**Backward compatibility:** Single conditions still work
- `cpu > 10` → parsed as `FilterExpr::Simple(Filter { ... })`
- No breaking changes to existing filters

**Forward compatibility:** Room for parentheses
- Phase 3 can add parentheses without breaking AND/OR
- `(cpu > 10 and mem > 5) or name == chrome`

## Performance Considerations

- Short-circuit evaluation: AND stops on first false, OR stops on first true
- Compiled once per watch iteration
- No regex needed, simple string split
- Expected overhead: <0.1ms per filter evaluation

## Documentation Updates

**README section:**
```markdown
## Multiple Filter Conditions

Combine conditions with `and` / `or` (case-insensitive):

# Both conditions must match
stop --filter "cpu > 50 and mem > 10"

# Either condition matches
stop --filter "name == chrome or name == firefox"

# Complex queries
stop --filter "cpu > 50 or mem > 10"
stop --filter "pid < 1000 and cpu > 5"
```

## Future Extensions (Phase 3)

- Parentheses for explicit grouping
- NOT operator (!) for negation
- IN operator for list matching: `name in [chrome, firefox]`
- Regex matching: `name =~ "^chrome.*"`
