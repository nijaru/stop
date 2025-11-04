# Strategic Roadmap

## Goal
Build a production-ready, cross-platform system monitoring tool with structured JSON output optimized for AI agents and automation.

**Target**: v1.0.0 by Q2 2025

## Current State (v0.0.1)

**Completed**: Phases 1, 2, and 4 ahead of schedule
- ✅ Core filtering with AND/OR logic
- ✅ Watch mode with NDJSON streaming
- ✅ CSV output (RFC 4180 compliant)
- ✅ 29 tests, zero clippy warnings
- ✅ Performance: 29ms overhead (< 100ms goal)

**Next**: Phase 3 (Advanced Monitoring) or field testing for v0.1.0

## Milestones

| Phase | Timeline | Status | Deliverables | Success Criteria |
|-------|----------|--------|--------------|------------------|
| Phase 1: MVP | Jan 2025 | ✅ COMPLETE | Filter, sort, tests, improved output | All CLI flags functional, test coverage >80% |
| Phase 2: Query & Filter | Feb 2025 | ✅ COMPLETE | Multiple conditions (AND/OR), CSV output | Complex queries work, CSV RFC 4180 compliant |
| Phase 3: Advanced Monitoring | Mar 2025 | ← CURRENT | Disk I/O, network, threads | All metrics accurate, cross-platform |
| Phase 4: Watch Mode | Feb 2025 | ✅ COMPLETE | Continuous monitoring, NDJSON | <100ms overhead, configurable interval |
| Phase 5: Production | Q2 2025 | Planned | Stable API, Windows support, publish | Published to crates.io, CI/CD complete |

## Critical Dependencies

| Dependency | Blocks | Reason |
|------------|--------|--------|
| Filter implementation | Phase 2 queries | Multiple conditions require filter foundation |
| Test suite | All phases | Need regression protection before expanding |
| Cross-platform validation | v1.0.0 | Can't release without Windows/Linux verification |

## Out of Scope (Deferred Post-v1.0)

- Real-time alerting system (users can build on top)
- Historical data storage (not a metrics DB)
- Process control/killing (security implications)
- Container-specific metrics (scope creep)
- Plugin system (premature complexity)

## Risks

| Risk | Impact | Mitigation |
|------|--------|-----------|
| sysinfo API limitations | High | Document limitations, consider direct syscalls for critical metrics |
| Windows compatibility | Medium | Early testing on Windows, may scope out for v1.0 |
| Filter parsing complexity | Low | Start simple (comparisons only), extend gradually |

## Success Metrics

**Phase 1-2 (COMPLETE)**:
- [x] All core CLI flags implemented (filter, sort, top-n, watch, CSV, JSON)
- [x] Test coverage >80% (29 tests: 16 unit + 13 integration)
- [x] Zero clippy warnings
- [x] Documented filter syntax (simple + compound AND/OR)
- [x] Performance verified: 29ms overhead (< 100ms goal)

**Phase 3 (Current Focus) - Implementation Order**:
1. [ ] Thread information (easiest, sysinfo API available)
   - Add thread count to ProcessInfo struct
   - Update JSON schema
   - Add tests
2. [ ] Disk I/O metrics per process (high value)
   - Add disk_read_bytes, disk_write_bytes to ProcessInfo
   - Use sysinfo disk_usage() API
   - Test on macOS and Linux (may have platform differences)
3. [ ] Network metrics per process (valuable but complex)
   - Add network_rx_bytes, network_tx_bytes to ProcessInfo
   - Research: sysinfo support? Platform-specific code needed?
   - May require /proc parsing on Linux, system calls on macOS
4. [ ] Open file descriptors/handles (nice-to-have)
   - Add open_files count to ProcessInfo
   - Platform-specific: /proc on Linux, lsof on macOS
   - Defer if too complex

**v1.0.0 Release**:
- [ ] 3+ platforms supported (macOS ✅, Linux ?, Windows ?)
- [ ] Published to crates.io
- [ ] Field tested by 5+ users
- [ ] Stable API documentation
