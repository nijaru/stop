# Strategic Roadmap

## Goal
Build a production-ready, cross-platform system monitoring tool with structured JSON output optimized for AI agents and automation.

**Target**: v1.0.0 by Q2 2025

## Milestones

| Phase | Timeline | Status | Deliverables | Success Criteria |
|-------|----------|--------|--------------|------------------|
| Phase 1: MVP | Jan 2025 | ← CURRENT | Filter, sort, tests, improved output | All CLI flags functional, test coverage >80% |
| Phase 2: Query & Filter | Feb 2025 | Planned | Multiple conditions, regex, CSV output | Complex queries work, CSV RFC 4180 compliant |
| Phase 3: Advanced Monitoring | Mar 2025 | Planned | Disk I/O, network, threads, connections | All metrics accurate, cross-platform |
| Phase 4: Polish | Apr 2025 | Planned | Watch mode, optimization, docs | <100ms overhead, comprehensive docs |
| Phase 5: Production | May 2025 | Planned | Stable API, Windows support, publish | Published to crates.io, CI/CD complete |

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

**Phase 1 (v0.1.0)**:
- [ ] All CLI flags implemented
- [ ] Test coverage >80%
- [ ] Zero clippy warnings
- [ ] Documented filter syntax

**v1.0.0 Release**:
- [ ] 3+ platforms supported
- [ ] Published to crates.io
- [ ] <100ms collection overhead
- [ ] 10+ users on GitHub
