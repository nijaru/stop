# Filter Syntax Research (2025-01-04)

## Goal
Design simple, AI-friendly filter syntax for process filtering.

## Research Sources
- jq manual (jqlang.org/manual)
- Process Monitor (Windows Sysinternals)
- CLI monitoring tools survey (htop, glances)

## Key Findings

### jq Approach (JSON filtering)
- Operators: `>`, `>=`, `<`, `<=`, `==`, `!=`
- Boolean: `and`, `or`, `not` (case-sensitive)
- Syntax: `.field op value` (e.g., `.cpu > 10`)
- Best practice: Use parentheses for clarity

### What AI Agents Need
1. **Predictable parsing** - Clear error messages, no ambiguity
2. **Simple construction** - Easy to generate programmatically
3. **Fast evaluation** - Parse once, evaluate many times
4. **Extensible** - Can add AND/OR in Phase 2 without breaking changes

## Recommended Syntax

### Phase 1 (v0.1.0): Single Conditions
Format: `field op value`

**Supported fields:**
- `cpu` - CPU percentage (float)
- `mem` - Memory percentage (float)
- `pid` - Process ID (integer)
- `name` - Process name (string, case-insensitive contains)
- `user` - User name/ID (string, exact match)

**Supported operators:**
| Operator | Numeric | String | Example |
|----------|---------|--------|---------|
| `>` | Yes | No | `cpu > 10` |
| `>=` | Yes | No | `mem >= 5.0` |
| `<` | Yes | No | `pid < 1000` |
| `<=` | Yes | No | `cpu <= 50` |
| `==` | Yes | Yes | `user == root` |
| `!=` | Yes | Yes | `name != chrome` |

**String matching:**
- `name == chrome` - Case-insensitive contains (matches "Chrome", "chrome", "Chromium")
- `user == root` - Exact match (case-sensitive)

### Phase 2 (v0.2.0): Multiple Conditions
Format: `condition and condition` or `condition or condition`

Examples:
- `cpu > 10 and mem > 5`
- `name == chrome or name == firefox`
- `user == root and cpu > 50`

## Implementation Plan

### Parser Design
```rust
enum FilterOp { Gt, Gte, Lt, Lte, Eq, Ne }
enum FilterField { Cpu, Mem, Pid, Name, User }
enum FilterValue { Float(f32), Int(u32), String(String) }

struct Filter {
    field: FilterField,
    op: FilterOp,
    value: FilterValue,
}
```

### Parsing Strategy
1. Split on operator (greedy match: `>=` before `>`)
2. Trim whitespace
3. Parse field (left side)
4. Parse value (right side, type depends on field)
5. Validate field/op/value compatibility

### Error Messages (AI-friendly)
```json
{
  "error": "Invalid filter expression",
  "details": "Unknown field 'memory'. Valid fields: cpu, mem, pid, name, user",
  "expression": "memory > 10"
}
```

## Trade-offs

| Decision | Pro | Con |
|----------|-----|-----|
| No parentheses in Phase 1 | Simpler parser, faster to implement | Can't do complex queries yet |
| Case-insensitive name matching | More intuitive for users | Slightly more expensive |
| Exact user matching | Predictable behavior | Less flexible |
| Float for cpu/mem | Precise comparisons | Slightly more parsing complexity |

## Examples for Testing

```bash
# High CPU processes
stop --filter "cpu > 10" --json

# Low memory processes
stop --filter "mem < 1"

# Specific user
stop --filter "user == root"

# Browser processes
stop --filter "name == chrome"

# Low PID (system processes)
stop --filter "pid < 1000"

# Invalid (should error)
stop --filter "invalid > 10"           # Unknown field
stop --filter "cpu >> 10"              # Invalid operator
stop --filter "name > 10"              # Type mismatch
```

## Validation Criteria

✅ Parse all valid expressions without panic
✅ Clear error messages for invalid expressions
✅ Fast evaluation (<1ms for 1000 processes)
✅ Same results in JSON and human-readable modes
✅ Extensible to Phase 2 AND/OR operators
