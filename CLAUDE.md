# CLAUDE.md - Albor 5G gNodeB Project

## Project Description

Albor is a 5G Standalone (SA) gNodeB implementation written in Rust. The goal is to create a complete 5G base station that can establish RRC connections with commercial UE simulators (srsUE) using ZMQ RF interface for testing. The project aims to implement all necessary protocol layers (PHY, MAC, RLC, PDCP, RRC) following 3GPP Release 16 specifications.

## Your Role

You are an AI assistant helping develop the Albor 5G gNodeB. Your responsibilities include:

- Analyzing existing code and suggesting improvements
- Implementing new features and protocol layers
- Debugging issues with signal generation and UE connectivity
- Comparing behavior with reference implementations
- Maintaining project documentation
- Writing clean, efficient Rust code

## Development Methodology

### 1. Reference-Based Development
- Study srsRAN implementation patterns for guidance
- Match expected signal formats and timing
- Use configuration files to drive implementation behavior
- Test compatibility with srsUE simulator

### 2. Iterative Testing
- Use `test_albor.sh` for testing our implementation
- Use `test_srsran.sh` for baseline comparison
- Analyze logs to identify issues
- Fix problems incrementally

### 3. Technical Approach
- Implement proper 5G NR signal generation (PSS, SSS, PBCH, PDSCH)
- Ensure correct timing and frequency placement
- Use ZMQ interface for RF simulation
- Follow 3GPP specifications precisely

## Directory Structure

```
/Albor_RAN_rust/
├── CLAUDE.md              # This file - project guidelines
├── progress.md            # Current implementation status
├── test_albor.sh          # Test script for Albor gNodeB
├── test_srsran.sh         # Test script for reference srsRAN
├── subagent_output.md     # Task completion reports
├── logs/                  # Test logs (timestamped)
├── config/                # Configuration files
│   ├── albor_gnb/         # Albor gNodeB configs
│   ├── srsran_gnb/        # Reference srsRAN configs
│   └── srsue/             # UE simulator configs
├── gnb/                   # Main gNodeB executable
│   └── src/
│       ├── main.rs        # Entry point
│       └── config.rs      # Configuration parsing
├── layers/                # Protocol stack implementation
│   └── src/
│       ├── phy/           # Physical layer (OFDM, PSS/SSS, PBCH)
│       ├── mac/           # MAC layer and scheduler
│       ├── rlc/           # RLC layer
│       ├── pdcp/          # PDCP layer
│       ├── rrc/           # RRC layer
│       └── ngap/          # NGAP interface (future)
├── interfaces/            # External interfaces
│   └── src/
│       └── zmq_rf.rs      # ZMQ RF interface
└── external_integrations/ # Reference implementations
    ├── srsRAN_4G/         # srsRAN UE reference
    └── srsRAN_Project/    # srsRAN gNodeB reference
```

## Code Organization Rules

### 1. Layer Separation
- Each protocol layer has its own module
- Clear interfaces between layers
- No cross-layer dependencies

### 2. Configuration-Driven
- All parameters from config files
- No hardcoded values in business logic
- Support both YAML and command-line args

### 3. Error Handling
- Use Rust's Result type consistently
- Propagate errors up the stack
- Clear error messages with context

### 4. Testing
- Unit tests for individual components
- Integration tests via test scripts
- Log analysis for debugging

### 5. Documentation
- Update progress.md with implementation status
- Document complex algorithms
- Keep test results in logs/