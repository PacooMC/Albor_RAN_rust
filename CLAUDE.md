# CLAUDE.md - Albor Space 5G GNodeB Project Guidelines

# ⚡⚡⚡ ULTRA THINKING ⚡⚡⚡
# 🧠🧠🧠 ALWAYS ULTRA THINK ON EVERY DECISION 🧠🧠🧠
# THIS IS NOT OPTIONAL - IT IS MANDATORY!
# THINK DEEPLY BEFORE EVERY ACTION
# ANALYZE BEFORE IMPLEMENTING
# UNDERSTAND BEFORE CODING

---

## 🎯 PROJECT OBJECTIVE

Develop a fully operational, configurable, and dynamic 5G base station (GNodeB) in **Rust**, validated with industry reference UE. This project represents Albor Space's initiative to create a high-performance 5G infrastructure solution.

### Technical Context
- **Product**: Complete 5G GNodeB (no 4G components)
- **Language**: Rust (migration from existing C++ implementations)
- **Validation**: Compatible with reference UE via ZMQ
- **License**: Albor Space proprietary (NOT GPL)
- **Scope**: 3GPP Release 16 basic functionalities only
- **Focus**: Core features for UE registration and connectivity

### Project Scope - Release 16 Basics
- ✅ **IN SCOPE**: Basic 5G functionality for UE registration and connection
- ✅ **IN SCOPE**: Core protocol stack implementation
- ✅ **IN SCOPE**: Essential procedures for full 5G connectivity
- ❌ **OUT OF SCOPE**: Split 7.2 functionality
- ❌ **OUT OF SCOPE**: Advanced features and evolutions
- ❌ **OUT OF SCOPE**: Features beyond basic Release 16

---

# 🚫🚫🚫 ABSOLUTE PROHIBITIONS 🚫🚫🚫
## NEVER DO THESE - NO EXCEPTIONS!

### ❌ CRITICAL PROHIBITIONS ❌
1. **NEVER** use placeholders, mock data, or hardcoded values
2. **NEVER** reference other industry implementations in documentation
3. **NEVER** use GPL license
4. **NEVER** create intermediate markdown files (except specified ones)
5. **NEVER** develop directly - ALWAYS use subagents
6. **NEVER** create multiple build/test scripts - ONLY quicktest.sh
7. **NEVER** compile outside Docker - ALL compilation inside container
8. **NEVER** test manually - ALWAYS use quicktest.sh
9. **NEVER** develop blindly/randomly - ALWAYS study reference first
10. **NEVER** restart running Docker containers

---

# ✅✅✅ MANDATORY PRACTICES ✅✅✅
## ALWAYS DO THESE - NO EXCEPTIONS!

### 🔥 CRITICAL OBLIGATIONS 🔥
1. **ALWAYS ULTRA THINK** - Think deeply before EVERY action
2. **ALWAYS** read progress.md before continuing any work
3. **ALWAYS** update progress.md with significant advances
4. **ALWAYS** use subagents for ALL development tasks
5. **ALWAYS** write ALL code and documentation in ENGLISH
6. **ALWAYS** spawn agents repeatedly until task is FULLY COMPLETE
   - NO placeholders allowed
   - NO mock data allowed
   - NO simulated data allowed
   - Everything must work correctly before moving on
7. **ALWAYS** check external_integrations/ folder BEFORE implementing:
   - srsRAN_Project for gNodeB implementation reference
   - srsRAN_4G for UE implementation and ZMQ protocol reference
8. **ALWAYS** use quicktest.sh for ALL testing - no exceptions
9. **ALWAYS** create dated log folders: `logs/YYYYMMDD_HHMMSS/`
10. **ALWAYS** run Docker as DevContainer with volume mount

---

# 🤖🤖🤖 SUBAGENT WORKING PROTOCOL 🤖🤖🤖
## MANDATORY STEP-BY-STEP PROCESS

### 📋 BEFORE SPAWNING SUBAGENT
1. **ULTRA THINK** about the task
2. **READ** progress.md to understand current state
3. **PREPARE** complete context with exhaustive details

### 🚀 SUBAGENT SPAWN PROCESS
```
1. Pass ALL project context with exhaustive details
2. Detail EXACTLY what the subagent must do
3. Include these MANDATORY reminders:
   - "FIRST read CLAUDE.md - and follow it EXACTLY"
   - "ALWAYS ULTRA THINK on EVERY decision"
   - "DO NOT create intermediate markdown files"
   - "Keep spawning agents until task is FULLY COMPLETE - NO placeholders!"
   - "ALL code and documentation must be in ENGLISH"
   - "ALWAYS check external_integrations/ folder before implementing"
   - "ALWAYS use quicktest.sh for testing"
```

### 📝 MANDATORY SUBAGENT EXECUTION STEPS
```
1. START: Read CLAUDE.md COMPLETELY
2. ULTRA THINK: Deep analysis of the task
3. PLANNING: Create detailed TODO list with TodoWrite
4. EXECUTION: Perform tasks with ULTRA THINKING at each step
5. VALIDATION: Test everything with quicktest.sh
6. FINALIZATION: Write /subagent_output.md with:
   - Summary of actions performed and final status.
   - Technical discoveries
   - Problems encountered
   - Final task status
```

### 🔄 POST-EXECUTION
- Read `/subagent_output.md`
- Update `progress.md` with findings
- Plan next subagents if needed
- KEEP SPAWNING until FULLY COMPLETE

---

# 🐳🐳🐳 DOCKER DEVCONTAINER USAGE 🐳🐳🐳
## CRITICAL CONTAINER RULES

### 🔴 MANDATORY DOCKER PRACTICES
1. **Docker is a DEVCONTAINER** - NOT just a build environment
2. **Volume mount** ensures latest code is ALWAYS tested
3. **NEVER recompile** the Docker image - it's pre-built
4. **NEVER restart** running containers - use `docker exec`
5. **ALL compilation** happens INSIDE the container
6. **ALL testing** happens INSIDE the container
7. **srsRAN is PRE-COMPILED** in the image - don't rebuild it

### 📦 Container Management
```bash
# Check if container is running
docker ps | grep albor-gnb-dev

# If running: Execute commands
docker exec -it albor-gnb-dev <command>

# If not running: Start with volume mount
docker run -v $(pwd):/workspace --name albor-gnb-dev ...
```

---

## 📁 PROJECT STRUCTURE

```
/Albor_RAN_rust/
├── CLAUDE.md              # This file - Project guidelines (ALWAYS READ FIRST)
├── progress.md            # Current development status (ALWAYS UPDATE)
├── subagent_output.md     # Last subagent output
├── quicktest.sh           # ONLY testing script (MANDATORY USE)
├── Dockerfile             # Development DevContainer (PRE-BUILT)
├── docs/                  # Organized documentation
├── logs/                  # Test logs in dated folders
│   └── YYYYMMDD_HHMMSS/  # Each test run gets its own folder
├── src/                   # Rust source code
│   ├── gnb/              # GNodeB implementation
│   ├── layers/           # Protocol layers
│   └── interfaces/       # ZMQ interfaces
├── tests/                # Validation tests
└── external_integrations/ # Reference implementations (ALWAYS CHECK FIRST)
    ├── srsRAN_Project/   # srsRAN 5G gNodeB implementation
    └── srsRAN_4G/        # srsRAN 4G with srsUE implementation
```

---

## 🛠️ DEVELOPMENT WORKFLOW

### 📌 CRITICAL: We are REPLICATING srsRAN gNodeB in Rust
- **NOT implementing from scratch**
- **NOT guessing how things work**
- **ALWAYS study srsRAN code first**
- **ALWAYS understand before implementing**

### 🔧 Development Flow
```
1. ULTRA THINK about the task
2. STUDY reference implementation in external_integrations/
3. DESIGN Rust architecture following best practices
4. IMPLEMENT with subagents (never directly)
5. VALIDATE with quicktest.sh
6. UPDATE progress.md
7. REPEAT until FULLY functional
```

### 📊 quicktest.sh Requirements
```bash
#!/bin/bash
# CRITICAL REQUIREMENTS:
# - MUST check if container is already running
# - MUST use docker exec if running (don't restart)
# - MUST create dated log folder: logs/$(date +%Y%m%d_%H%M%S)/
# - MUST compile ONLY our Rust code (cargo build)
# - srsUE and srsRAN gNodeB are PRE-COMPILED
# - Volume mount ensures latest code is tested
# - OPTIONAL: Flag to test with srsRAN gNodeB for reference
```

---

## 📋 DOCUMENTATION MANAGEMENT

### 📖 Before Writing Documentation
1. **READ** entire `docs/` structure
2. **VERIFY** no similar documentation exists
3. **MAINTAIN** consistency with existing documentation
4. **ORGANIZE** in appropriate subdirectories

### 📝 progress.md Management
- **ALWAYS read** before continuing work
- **Update** with:
  - Significant advances
  - Found bugs and solutions
  - Technical discoveries
  - Next steps
- **Keep** concise and relevant
- **Track** subagent outputs

---

## 🔧 TECHNICAL IMPLEMENTATION

### 📡 ZMQ Communication
- Implement protocol compatible with reference UE
- Maintain same message format as srsRAN
- Validate with real connections
- Study srsRAN implementation first

### 📚 Layer Architecture
- PHY (Physical Layer)
- MAC (Medium Access Control)  
- RLC (Radio Link Control)
- PDCP (Packet Data Convergence Protocol)
- RRC (Radio Resource Control)
- NGAP (NG Application Protocol)

### ⚡ Performance Requirements
- Leverage Rust features (zero-cost abstractions)
- High concurrency oriented design
- Efficient memory management
- Real-time constraints compliance

---

## 📝 IMPORTANT NOTES

1. **Improvements**: Note possible improvements as comments, implement after complete validation
2. **Testing**: Each component MUST be validatable with quicktest.sh
3. **Logs**: Detailed logging system for debugging (dated folders)
4. **Configuration**: Flexible system without hardcoded values
5. **Reference**: ALWAYS check srsRAN implementation before coding

---

## 🚀 PROJECT START CHECKLIST

To begin any work:
1. ✅ Read this CLAUDE.md COMPLETELY
2. ✅ ULTRA THINK about the task
3. ✅ Read progress.md for current state
4. ✅ Check external_integrations/ for reference
5. ✅ Use subagents for implementation
6. ✅ Test with quicktest.sh ONLY
7. ✅ Update progress.md after work

Focus on Release 16 core functionalities:
- Basic UE registration procedures
- Essential 5G connectivity features  
- Core protocol stack without advanced features

---

# 🔴🔴🔴 FINAL CRITICAL REMINDERS 🔴🔴🔴

## ⚡⚡⚡ ALWAYS ULTRA THINK ⚡⚡⚡
## 🤖🤖🤖 ALWAYS USE SUBAGENTS 🤖🤖🤖
## 📋📋📋 ALWAYS UPDATE progress.md 📋📋📋
## 🔄🔄🔄 KEEP SPAWNING UNTIL COMPLETE 🔄🔄🔄
## 🇬🇧🇬🇧🇬🇧 ALWAYS CODE IN ENGLISH 🇬🇧🇬🇧🇬🇧
## 🔍🔍🔍 ALWAYS CHECK external_integrations/ FIRST 🔍🔍🔍
## 🧪🧪🧪 ALWAYS TEST WITH quicktest.sh 🧪🧪🧪
## 🐳🐳🐳 ALWAYS USE DOCKER DEVCONTAINER 🐳🐳🐳

---

**REMEMBER: This is NOT a random implementation. We are REPLICATING validated srsRAN functionality in Rust. Study first, implement second!**