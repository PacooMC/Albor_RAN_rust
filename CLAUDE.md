# CLAUDE.md - Albor Space 5G GNodeB Project Guidelines

# ⚡⚡⚡ ULTRATHINKING ⚡⚡⚡
# 🧠🧠🧠 ALWAYS ULTRATHINK ON EVERY DECISION 🧠🧠🧠
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
1. **ALWAYS ULTRATHINK** - Think deeply before EVERY action
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
1. **ULTRATHINK** about the task
2. **READ** progress.md to understand current state
3. **PREPARE** complete context with exhaustive details

### 🚀 SUBAGENT SPAWN PROCESS
```
1. Pass ALL project context with exhaustive details
2. Detail EXACTLY what the subagent must do
3. Include these MANDATORY reminders:
   - "FIRST read CLAUDE.md - and follow it EXACTLY"
   - "ALWAYS ULTRATHINK on EVERY decision"
   - "DO NOT create intermediate markdown files"
   - "Keep spawning agents until task is FULLY COMPLETE - NO placeholders!"
   - "ALL code and documentation must be in ENGLISH"
   - "ALWAYS check external_integrations/ folder before implementing"
   - "ALWAYS use quicktest.sh for testing"
```

### 📝 MANDATORY SUBAGENT EXECUTION STEPS
```
1. START: Read CLAUDE.md COMPLETELY
2. ULTRATHINK: Deep analysis of the task
3. PLANNING: Create detailed TODO list with TodoWrite
4. EXECUTION: Perform tasks with ULTRATHINKING at each step
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
1. ULTRATHINK about the task
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
2. ✅ ULTRATHINK about the task
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

## ⚡⚡⚡ ALWAYS ULTRATHINK ⚡⚡⚡
## 🤖🤖🤖 ALWAYS USE SUBAGENTS 🤖🤖🤖
## 📋📋📋 ALWAYS UPDATE progress.md 📋📋📋
## 🔄🔄🔄 KEEP SPAWNING UNTIL COMPLETE 🔄🔄🔄
## 🇬🇧🇬🇧🇬🇧 ALWAYS CODE IN ENGLISH 🇬🇧🇬🇧🇬🇧
## 🔍🔍🔍 ALWAYS CHECK external_integrations/ FIRST 🔍🔍🔍
## 🧪🧪🧪 ALWAYS TEST WITH quicktest.sh 🧪🧪🧪
## 🐳🐳🐳 ALWAYS USE DOCKER DEVCONTAINER 🐳🐳🐳

---

# 📡📡📡 5G SA CONFIGURATION REFERENCE 📡📡📡
## MANDATORY NETWORK PARAMETERS

### 🔧 Core Network Configuration
- **Open5GS**: Deployed via Docker Compose in `config/open5gs/`
- **PLMN**: 00101 (MCC=001, MNC=01)
- **TAC**: 7
- **AMF N2 Interface**: Port 38412 (SCTP)
- **WebUI**: Port 9999

### 📻 Radio Configuration
- **Band**: 3 (1800 MHz FDD)
- **DL ARFCN**: 368500 (1842.5 MHz)
- **Bandwidth**: 10 MHz (52 PRBs)
- **Sub-Carrier Spacing**: 15 kHz (FDD only)
- **Sample Rate**: 23.04 MHz
- **FFT Size**: 1024
- **CP Length**: 72 samples (normal CP)

### 🔌 ZMQ Interface Configuration
```
gNodeB TX → tcp://127.0.0.1:2000 → UE RX
gNodeB RX ← tcp://127.0.0.1:2001 ← UE TX
```

### 📋 Reference Configuration Files
All configuration files are located in the `config/` directory:
```
config/
├── documentation/
│   └── 5g_sa_setup.md         # Complete 5G SA documentation
├── open5gs/
│   ├── docker-compose.yml     # 5G Core deployment
│   ├── config/amf.yaml        # AMF with PLMN 00101, TAC 7
│   └── README.md              # Open5GS setup guide
├── srsran_gnb/
│   └── gnb_zmq.yml           # Reference gNodeB configuration
└── srsue/
    └── ue_nr_zmq.conf        # srsUE NR mode configuration
```

### ⚙️ Critical Parameters Our Implementation MUST Match
1. **SSB Transmission**: Every 20 ms
2. **SIB1 Period**: Every 160 ms (16 radio frames)
3. **CORESET#0 Index**: 12 (for band 3, 10 MHz)
4. **PRACH Config Index**: 0 (FDD format)
5. **Cell ID**: 1
6. **DMRS Positions**: 
   - PDCCH: Subcarriers 1, 5, 9 in each RB
   - PDSCH: Type 1, alternating pattern

### 🚀 Quick Start Commands
```bash
# 1. Start Open5GS Core
cd config/open5gs
docker-compose up -d

# 2. Run reference gNodeB (for testing)
cd /opt/srsRAN_Project/build
./apps/gnb/gnb -c /workspace/config/srsran_gnb/gnb_zmq.yml

# 3. Run srsUE
cd /opt/srsRAN_4G/build
./srsue/src/srsue /workspace/config/srsue/ue_nr_zmq.conf
```

### 📊 Validation Checklist
- [ ] UE detects cell: "Found Cell: PCI=1, PRB=52"
- [ ] PRACH successful: "Random Access Complete"
- [ ] Registration complete: "PDU Session Establishment successful"
- [ ] Data plane active: IP connectivity established

### 🔧 Complete 5G SA Test Setup
```bash
# Complete test script available
./config/test_5g_sa_setup.sh          # Test with srsRAN Project gNodeB
./config/test_5g_sa_setup.sh --use-our-gnb  # Test with our implementation
```

### 📡 Key Technical Parameters
**Physical Layer**:
- PSS/SSS in subcarriers -31 to 31 (center 6 RBs)
- PBCH spans 20 RBs around DC
- CORESET#0: 48 RBs starting from RB 0
- PDCCH scrambled with SI-RNTI (0xFFFF) for SIB1
- DMRS amplitude: 0.7071 (1/sqrt(2))

**MAC Layer**:
- SIB1 size: ~100 bytes with RACH config
- SI window: 20 ms
- PRACH occasions: frame%16==1, slot 9 (FDD)
- RA response window: 10 slots

**Network Configuration**:
- Open5GS network: 10.53.1.0/24
- AMF: 10.53.1.2:38412
- UPF: 10.53.1.7 (N3 interface)
- MongoDB: 10.53.1.100
- DN subnet: 10.45.0.0/16

### 📋 Testing Workflow
1. **Deploy Open5GS**: `docker-compose up -d` in config/open5gs/
2. **Check AMF**: Verify NGAP listening on port 38412
3. **Start gNodeB**: Must see "N2: Connection to AMF" in logs
4. **Start UE**: Watch for cell search, PRACH, RRC, and registration
5. **Verify**: Check for assigned IP and ping connectivity

### 🚨 Common Issues & Solutions
- **Port conflicts**: Kill processes on 2000/2001 before ZMQ test
- **SCTP issues**: Ensure kernel has SCTP support loaded
- **Network namespace**: Use for UE isolation (optional)
- **Timing**: Allow 10s for Open5GS initialization

---

**REMEMBER: This is NOT a random implementation. We are REPLICATING validated srsRAN functionality in Rust. Study first, implement second!**