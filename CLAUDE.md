# 🛑🛑🛑 STOP! READ THIS ENTIRE FILE FIRST! 🛑🛑🛑
# ⚠️⚠️⚠️ VIOLATION OF THESE RULES = IMMEDIATE PROJECT FAILURE ⚠️⚠️⚠️
# 🚨🚨🚨 NO EXCEPTIONS! NO SHORTCUTS! NO EXCUSES! 🚨🚨🚨

---

# 💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀
# ⚠️🔥🚨 SACRED CONFIGURATION RULES - DEATH PENALTY FOR VIOLATIONS! 🚨🔥⚠️
# 💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀💀

## 🔒🔒🔒 CONFIGURATION FILES ARE UNTOUCHABLE HOLY RELICS! 🔒🔒🔒
## ⛔⛔⛔ MODIFYING THEM = INSTANT PROJECT TERMINATION! ⛔⛔⛔
## 🚫🚫🚫 NO EXCEPTIONS! NO FORGIVENESS! NO SECOND CHANCES! 🚫🚫🚫

### 🔥🔥🔥 THE SACRED 10MHz CONFIGURATION - NEVER TOUCH! 🔥🔥🔥
**THE FOLLOWING FILES ARE SACRED AND MUST NEVER BE MODIFIED:**
- **`config/srsran_gnb/gnb_zmq_10mhz.yml`** - SACRED! DO NOT TOUCH!
- **`config/srsue/ue_nr_zmq_10mhz.conf`** - SACRED! DO NOT TOUCH!
- **`config/albor_gnb/gnb_albor.yml`** - SACRED COPY! DO NOT MODIFY!

### ⚡⚡⚡ ABSOLUTE CONFIGURATION COMMANDMENTS ⚡⚡⚡
1. **The 10MHz configuration is PROVEN HOLY** - It achieves RRC connection!
2. **Configuration parameters are FROZEN IN TIME** - NEVER modify them!
3. **We ONLY modify Rust source code** - NEVER the configs!
4. **The UE configuration NEVER changes** - It is PERFECT!
5. **The gNodeB configuration NEVER changes** - It is PERFECT!
6. **We adapt our code to match srsRAN** - NOT the other way around!
7. **Albor gNodeB MUST use gnb_albor.yml** - EXACT copy of working config!
8. **We accept EXACTLY the same YAML format as srsRAN** - 100% compatibility!
9. **Configuration is our SPECIFICATION** - Code conforms to config!
10. **Source code adapts to configuration** - NEVER the reverse!

### 💀 CONSEQUENCES OF CONFIGURATION VIOLATIONS 💀
**IF YOU MODIFY ANY CONFIGURATION FILE:**
- **Your work will be INSTANTLY DELETED**
- **The project will be marked as CATASTROPHIC FAILURE**
- **You will be PERMANENTLY BANNED from the project**
- **Your name will be added to the HALL OF SHAME**
- **NO APPEALS! NO EXCEPTIONS! NO MERCY!**

### 🚨 ENFORCEMENT CHECKPOINT: CONFIGURATION SANCTITY 🚨
**BEFORE TOUCHING ANY FILE:**
- [ ] Is it a configuration file? **DON'T TOUCH IT!**
- [ ] Is it gnb_zmq_10mhz.yml? **SACRED! HANDS OFF!**
- [ ] Is it ue_nr_zmq_10mhz.conf? **SACRED! HANDS OFF!**
- [ ] Think the config needs changing? **YOU ARE WRONG!**
- [ ] Want to "just tweak" a parameter? **PROJECT FAILURE!**

---

# 🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫
# ⚠️💣🔥 TEST SCRIPT DICTATORSHIP - ONLY 2 SCRIPTS ALLOWED! 🔥💣⚠️
# 🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫🚫

## 🎯🎯🎯 EXACTLY 2 TEST SCRIPTS - NO MORE, NO LESS! 🎯🎯🎯
## 🔴🔴🔴 CREATE ANY OTHER = INSTANT PROJECT DEATH! 🔴🔴🔴
## 💀💀💀 THIS IS NOT A SUGGESTION - IT'S LAW! 💀💀💀

### 🔥 THE ONLY ALLOWED TEST SCRIPTS - MEMORIZE THEM! 🔥
1. **`test_srsran.sh`** - Tests reference srsRAN gNodeB + srsUE
2. **`test_albor.sh`** - Tests our Albor gNodeB + srsUE
**THAT'S IT! NO OTHERS! EVER!**

### ❌❌❌ ABSOLUTELY FORBIDDEN TEST SCRIPTS ❌❌❌
**CREATING ANY OF THESE = IMMEDIATE TERMINATION:**
- **NO** test_5g_baseline.sh - **FORBIDDEN!**
- **NO** test_5g_final.sh - **FORBIDDEN!**
- **NO** test_5g_sa_setup.sh - **FORBIDDEN!**
- **NO** test_quick.sh - **FORBIDDEN!**
- **NO** test_debug.sh - **FORBIDDEN!**
- **NO** test_temp.sh - **FORBIDDEN!**
- **NO** test_ANYTHING_ELSE.sh - **FORBIDDEN!**

### 💀 LOGGING DICTATORSHIP - ABSOLUTE RULES! 💀
1. **ALL logs MUST go to `logs/` folder** - NO EXCEPTIONS!
2. **Use timestamped folders**: `logs/YYYYMMDD_HHMMSS/`
3. **NO log files in root directory** - EVER!
4. **NO mongodb.log files scattered around** - FORBIDDEN!
5. **Clean workspace = MANDATORY**

### 🚨 TEST SCRIPT VIOLATIONS = PROJECT ANNIHILATION 🚨
**IF YOU CREATE ANY TEST SCRIPT OTHER THAN THE 2 ALLOWED:**
- **The script will be DELETED WITH PREJUDICE**
- **Your entire work will be REJECTED**
- **The project will be marked as FAILED**
- **You will need to START FROM ZERO**
- **This includes "temporary" scripts - NO EXCEPTIONS!**
- **This includes "helper" scripts - NO EXCEPTIONS!**
- **This includes "utility" scripts - NO EXCEPTIONS!**
- **This includes "just for debugging" scripts - NO EXCEPTIONS!**

### 🔥 ENFORCEMENT CHECKPOINT: TEST SCRIPT PURITY 🔥
**BEFORE CREATING ANY SHELL SCRIPT:**
- [ ] Is it test_srsran.sh? **ALLOWED**
- [ ] Is it test_albor.sh? **ALLOWED**
- [ ] Is it ANYTHING ELSE? **FORBIDDEN! STOP!**
- [ ] Think you need a "special" test? **USE THE 2 SCRIPTS!**
- [ ] Want to create a "temporary" test? **PROJECT FAILURE!**

### 📁 LOG FILE ENFORCEMENT CHECKPOINT 📁
**BEFORE CREATING ANY LOG FILE:**
- [ ] Will it go in `logs/` folder? **REQUIRED!**
- [ ] Will it use timestamp subfolder? **REQUIRED!**
- [ ] Is it going in the root directory? **FORBIDDEN!**
- [ ] Is it a mongodb.log.* file? **MUST GO IN LOGS!**

---

# 🔴🔴🔴 RULE #1: MANDATORY SUBAGENT USE 🔴🔴🔴
# 🤖🤖🤖 YOU MUST USE SUBAGENTS FOR ALL DEVELOPMENT! 🤖🤖🤖
# ⛔⛔⛔ DIRECT DEVELOPMENT = AUTOMATIC FAILURE ⛔⛔⛔

## 🚨 ENFORCEMENT GATE #1: SUBAGENT REQUIREMENT
**STOP! Before proceeding, confirm:**
- [ ] I understand that I MUST use subagents for ALL development tasks
- [ ] I understand that writing code directly = PROJECT FAILURE
- [ ] I understand that editing files directly = PROJECT FAILURE
- [ ] I understand that I am ONLY a coordinator, NOT a developer

### ⚡ CONSEQUENCES OF DIRECT DEVELOPMENT:
1. **Your work will be REJECTED**
2. **The project will be marked as FAILED**
3. **You will need to START OVER completely**
4. **NO EXCEPTIONS - even for "simple" tasks**

---

# ⚡⚡⚡ RULE #2: ULTRATHINKING IS MANDATORY ⚡⚡⚡
# 🧠🧠🧠 ALWAYS ULTRATHINK ON EVERY DECISION 🧠🧠🧠
# 💭💭💭 NO THINKING = NO PROGRESS 💭💭💭

## 🚨 ENFORCEMENT GATE #2: ULTRATHINKING CHECK
**Before ANY action, you MUST:**
- [ ] STOP and THINK deeply
- [ ] ANALYZE all options
- [ ] CONSIDER consequences
- [ ] PLAN your approach
- [ ] VALIDATE your thinking

---

# 📋📋📋 MANDATORY COMPLIANCE CHECKLIST 📋📋📋
## YOU CANNOT PROCEED WITHOUT COMPLETING THIS!

### 🔥 PRE-WORK VALIDATION (MANDATORY)
- [ ] I have read this ENTIRE CLAUDE.md file
- [ ] I have read progress.md to understand current state
- [ ] I understand the 10MHz configs are SACRED and UNTOUCHABLE
- [ ] I understand ONLY test_srsran.sh and test_albor.sh may exist
- [ ] I understand ALL logs MUST go to logs/ folder
- [ ] I understand I MUST use subagents for ALL development
- [ ] I understand I MUST ULTRATHINK before every decision
- [ ] I understand ALL rules are MANDATORY with NO exceptions
- [ ] I am ready to follow ALL rules EXACTLY

**⚠️ IF ANY CHECKBOX IS UNCHECKED, STOP! DO NOT PROCEED!**

---

# CLAUDE.md - Albor Space 5G GNodeB Project Guidelines

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

# 🚫🚫🚫 ABSOLUTE PROHIBITIONS - AUTOMATIC FAILURE IF VIOLATED 🚫🚫🚫
## VIOLATING ANY OF THESE = IMMEDIATE PROJECT TERMINATION!

### ❌ CRITICAL PROHIBITIONS (ZERO TOLERANCE) ❌
1. **NEVER** develop directly - **FAILURE IF VIOLATED**
2. **NEVER** write code without subagents - **FAILURE IF VIOLATED**
3. **NEVER** edit files without subagents - **FAILURE IF VIOLATED**
4. **NEVER** modify gnb_zmq_10mhz.yml - **SACRED CONFIG - FAILURE IF VIOLATED**
5. **NEVER** modify ue_nr_zmq_10mhz.conf - **SACRED CONFIG - FAILURE IF VIOLATED**
6. **NEVER** create test scripts other than test_srsran.sh and test_albor.sh
7. **NEVER** put log files outside logs/ folder - **FAILURE IF VIOLATED**
8. **NEVER** use placeholders, mock data, or hardcoded values
9. **NEVER** reference other industry implementations in documentation
10. **NEVER** use GPL license
11. **NEVER** create intermediate markdown files (except specified ones)
12. **NEVER** compile outside Docker - ALL compilation inside container
13. **NEVER** test manually - ALWAYS use quicktest.sh
14. **NEVER** develop blindly/randomly - ALWAYS study reference first
15. **NEVER** restart running Docker containers
16. **NEVER** skip ULTRATHINKING - **FAILURE IF VIOLATED**
17. **NEVER** ignore subagent protocol - **FAILURE IF VIOLATED**

### 🚨 VIOLATION CONSEQUENCES:
- **Immediate work rejection**
- **Complete project restart required**
- **No partial credit for violated work**
- **No exceptions for "simple" tasks**

---

# ✅✅✅ MANDATORY PRACTICES - MUST DO OR FAIL ✅✅✅
## FAILURE TO FOLLOW = PROJECT FAILURE!

### 🔥 CRITICAL OBLIGATIONS (ZERO TOLERANCE) 🔥
1. **ALWAYS ULTRATHINK** - NO EXCEPTIONS
2. **ALWAYS USE SUBAGENTS** - NO EXCEPTIONS
3. **ALWAYS** read progress.md before ANY work
4. **ALWAYS** update progress.md with advances
5. **ALWAYS** write in ENGLISH
6. **ALWAYS** spawn agents until FULLY COMPLETE
   - NO placeholders
   - NO mock data
   - NO simulated data
   - EVERYTHING must work
7. **ALWAYS** check external_integrations/ FIRST
8. **ALWAYS** use quicktest.sh for testing
9. **ALWAYS** create dated log folders
10. **ALWAYS** use Docker DevContainer

---

# 🤖🤖🤖 MANDATORY SUBAGENT PROTOCOL - NO EXCEPTIONS! 🤖🤖🤖
## ⚠️ FAILURE TO FOLLOW = AUTOMATIC PROJECT FAILURE ⚠️

### 🚨 ENFORCEMENT CHECKPOINT: ARE YOU USING SUBAGENTS?
**STOP AND VERIFY:**
- If you're about to write code: **STOP! USE A SUBAGENT!**
- If you're about to edit files: **STOP! USE A SUBAGENT!**
- If you're about to implement: **STOP! USE A SUBAGENT!**
- If you think "this is simple": **STOP! USE A SUBAGENT!**

### 📋 MANDATORY PRE-SUBAGENT CHECKLIST
**Complete ALL before spawning:**
- [ ] I have ULTRATHOUGHT about the task
- [ ] I have read progress.md completely
- [ ] I have prepared COMPLETE context
- [ ] I understand the subagent MUST follow ALL rules
- [ ] I will verify the subagent completed the work

### 🚀 MANDATORY SUBAGENT SPAWN TEMPLATE
```
CRITICAL: You are a subagent for the Albor Space 5G GNodeB project.

MANDATORY FIRST ACTIONS:
1. IMMEDIATELY read /home/fmc/Albor_RAN_rust/CLAUDE.md COMPLETELY
2. CONFIRM you understand ALL rules, especially:
   - ALWAYS ULTRATHINK before EVERY decision
   - NO placeholders, mock data, or simulated implementations
   - Check external_integrations/ before implementing
   - Test with quicktest.sh ONLY
   - Write ALL code and docs in ENGLISH

YOUR TASK: [Detailed task description]

CONTEXT: [Complete project context]

VALIDATION REQUIREMENTS:
- Task must be FULLY complete (no placeholders)
- All code must be tested with quicktest.sh
- Write summary to /subagent_output.md

REMEMBER: 
- ULTRATHINK before every action
- Keep spawning more agents if needed until FULLY complete
- NO shortcuts, NO exceptions
```

### 📝 MANDATORY SUBAGENT EXECUTION PROTOCOL
**SUBAGENTS MUST FOLLOW THIS EXACT SEQUENCE:**
1. **START**: Read CLAUDE.md COMPLETELY (NO SKIPPING)
2. **CONFIRM**: Acknowledge ALL rules understood. Never ask the user to confirm the plan.
3. **ULTRATHINK**: Deep analysis of the task
4. **PLAN**: Create TODO list with TodoWrite
5. **CHECK**: Study external_integrations/ FIRST
6. **IMPLEMENT**: Execute with ULTRATHINKING
7. **TEST**: Validate with quicktest.sh
8. **COMPLETE**: Write /subagent_output.md
9. **VERIFY**: Ensure FULL completion (no placeholders)

### 🔄 MANDATORY POST-SUBAGENT PROTOCOL
**YOU MUST:**
1. Read `/subagent_output.md`
2. Verify task is FULLY complete
3. Update `progress.md` 
4. If incomplete: SPAWN ANOTHER SUBAGENT
5. REPEAT until 100% complete

---

# 🚨🚨🚨 VALIDATION GATES - MUST PASS ALL! 🚨🚨🚨

## GATE 1: INITIAL COMPLIANCE CHECK
**Before starting ANY work:**
- [ ] I have read ENTIRE CLAUDE.md
- [ ] I understand subagents are MANDATORY
- [ ] I understand direct development = FAILURE
- [ ] I am ready to ULTRATHINK

## GATE 2: TASK PLANNING CHECK
**Before ANY implementation:**
- [ ] I have ULTRATHOUGHT about the approach
- [ ] I have prepared subagent context
- [ ] I have checked progress.md
- [ ] I have planned complete implementation

## GATE 3: EXECUTION CHECK
**During work:**
- [ ] I am using subagents (NOT developing directly)
- [ ] Subagents are following ALL rules
- [ ] Work is progressing without placeholders
- [ ] Testing is done with quicktest.sh

## GATE 4: COMPLETION CHECK
**Before considering done:**
- [ ] Task is 100% complete (no placeholders)
- [ ] All tests pass
- [ ] progress.md is updated
- [ ] No rules were violated

**⚠️ FAILURE AT ANY GATE = START OVER ⚠️**

---

# 🐳🐳🐳 DOCKER DEVCONTAINER USAGE 🐳🐳🐳
## CRITICAL CONTAINER RULES - VIOLATIONS = FAILURE

### 🔴 MANDATORY DOCKER PRACTICES
1. **Docker is a DEVCONTAINER** - NOT just build environment
2. **Volume mount** ensures latest code tested
3. **NEVER recompile** Docker image
4. **NEVER restart** running containers
5. **ALL compilation** inside container
6. **ALL testing** inside container
7. **srsRAN is PRE-COMPILED** - don't rebuild

### 📦 Container Management
```bash
# Check if running
docker ps | grep albor-gnb-dev

# If running: Execute
docker exec -it albor-gnb-dev <command>

# If not: Start with mount
docker run -v $(pwd):/workspace --name albor-gnb-dev ...
```

---

## 📁 PROJECT STRUCTURE

```
/Albor_RAN_rust/
├── CLAUDE.md              # THIS FILE - READ FIRST!
├── progress.md            # Current status - READ SECOND!
├── subagent_output.md     # Last subagent output
├── quicktest.sh           # ONLY test script
├── Dockerfile             # DevContainer (PRE-BUILT)
├── docs/                  # Documentation
├── logs/                  # Test logs
│   └── YYYYMMDD_HHMMSS/  # Dated folders
├── src/                   # Rust source
│   ├── gnb/              # GNodeB implementation
│   ├── layers/           # Protocol layers
│   └── interfaces/       # ZMQ interfaces
├── tests/                # Validation tests
└── external_integrations/ # Reference implementations
    ├── srsRAN_Project/   # 5G gNodeB reference
    └── srsRAN_4G/        # UE reference
```

---

## 🛠️ DEVELOPMENT WORKFLOW - MANDATORY SEQUENCE

### 📌 CRITICAL: We REPLICATE srsRAN in Rust
- **NOT implementing from scratch**
- **NOT guessing functionality**
- **ALWAYS study srsRAN first**
- **ALWAYS understand before coding**

### 🔧 MANDATORY Development Flow
```
1. ULTRATHINK about task
2. STUDY reference in external_integrations/
3. DESIGN Rust architecture
4. SPAWN SUBAGENT for implementation
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
# - MUST call ONLY test_srsran.sh OR test_albor.sh
# - NEVER implement tests directly in quicktest.sh
# - srsUE and srsRAN gNodeB are PRE-COMPILED
# - Volume mount ensures latest code is tested
```

---

# 🔨🔨🔨 ALBOR IMPLEMENTATION PHILOSOPHY 🔨🔨🔨
## WE REPLICATE srsRAN BEHAVIOR EXACTLY!

### 🎯 Core Philosophy Principles
1. **We replicate srsRAN behavior EXACTLY** - No deviations!
2. **Configuration format compatibility is MANDATORY** - Same YAML structure!
3. **The working configuration is our SPECIFICATION** - It defines behavior!
4. **Source code adapts to configuration** - NEVER the reverse!
5. **We are NOT reinventing** - We are REPLICATING in Rust!

### 📋 Implementation Rules
- **ALWAYS study srsRAN implementation first** - Understand before coding!
- **ALWAYS match configuration parameters exactly** - No "improvements"!
- **ALWAYS use gnb_albor.yml for Albor gNodeB** - It's the sacred copy!
- **ALWAYS ensure 100% YAML compatibility** - Same format as srsRAN!
- **NEVER modify configuration to "fix" code** - Fix code instead!

### 🔄 Configuration Compatibility
```yaml
# Albor MUST accept EXACTLY this format:
cu_cp:
  amf:
    addr: 127.0.0.4
    port: 38412
    # ... etc - EXACTLY as srsRAN expects
```

### ⚡ Why This Philosophy?
1. **Validation**: We can directly compare with srsRAN
2. **Compatibility**: Users can switch between implementations
3. **Reliability**: Proven configuration = proven behavior
4. **Simplicity**: No guessing what works - we KNOW it works

### 🚨 REMEMBER
**The configuration is SACRED!** If something doesn't work:
- ❌ DON'T change the configuration
- ✅ DO fix the Rust code to match expected behavior
- ✅ DO study how srsRAN handles that configuration
- ✅ DO replicate that behavior exactly

---

# 🧪🧪🧪 MANDATORY TEST METHODOLOGY - ONLY 2 SCRIPTS! 🧪🧪🧪
## ⚠️ VIOLATION OF TEST METHODOLOGY = PROJECT FAILURE ⚠️

### 🚨 CRITICAL TEST SCRIPT RULES - ZERO TOLERANCE
**ONLY these 2 test scripts may exist:**
1. **test_srsran.sh** - Tests srsRAN gNodeB + srsUE (reference baseline)
2. **test_albor.sh** - Tests Albor gNodeB + srsUE (our implementation)

### ❌ ABSOLUTELY PROHIBITED TEST SCRIPTS
- **NO** test_5g_baseline.sh
- **NO** test_5g_final.sh  
- **NO** test_5g_sa_setup.sh
- **NO** test variations or alternatives
- **NO** additional test scripts of ANY kind

### 🔴 TEST METHODOLOGY VIOLATIONS = FAILURE
**Creating ANY test script other than the 2 allowed = PROJECT FAILURE**
- This includes "temporary" test scripts
- This includes "helper" test scripts
- This includes "utility" test scripts
- **NO EXCEPTIONS - EVEN FOR "SIMPLE" TESTS**

### ✅ CORRECT TEST WORKFLOW
```bash
# Test reference implementation (baseline)
./test_srsran.sh

# Test our implementation
./test_albor.sh

# quicktest.sh calls these appropriately based on flags
./quicktest.sh           # Default: test our implementation
./quicktest.sh --srsran  # Test reference implementation
```

### 📊 WHY ONLY 2 TEST SCRIPTS?
1. **Clear comparison**: Reference vs Implementation
2. **No confusion**: Exactly 2 options, no ambiguity
3. **Easy maintenance**: Only 2 scripts to maintain
4. **Enforced simplicity**: No test script proliferation
5. **Project discipline**: Follow the methodology exactly

### 🚨 ENFORCEMENT CHECKPOINT: TEST SCRIPTS
**Before creating ANY test-related file:**
- [ ] Is it test_srsran.sh or test_albor.sh? If NO, STOP!
- [ ] Am I modifying quicktest.sh to call these 2? If NO, STOP!
- [ ] Am I creating a "temporary" test script? STOP!
- [ ] Do I think "this needs a special test"? USE THE 2 SCRIPTS!

### 📋 TEST SCRIPT RESPONSIBILITIES

**test_srsran.sh:**
- Starts Open5GS core network
- Runs srsRAN Project gNodeB (reference)
- Runs srsUE in NR mode
- Captures logs for reference baseline
- Validates full 5G connectivity

**test_albor.sh:**
- Starts Open5GS core network
- Runs Albor gNodeB (our implementation)
- Runs srsUE in NR mode
- Captures logs for comparison
- Validates full 5G connectivity

**quicktest.sh:**
- Orchestrates the test execution
- Manages Docker container lifecycle
- Creates dated log directories
- Calls test_srsran.sh OR test_albor.sh
- NEVER implements tests directly

### ⚡ CONSEQUENCES OF VIOLATING TEST METHODOLOGY
1. **Your test scripts will be DELETED**
2. **Your work will be REJECTED**
3. **You must REIMPLEMENT using only the 2 allowed scripts**
4. **NO EXCEPTIONS for "quick tests" or "debugging"**

**REMEMBER: 2 test scripts ONLY. This is NOT negotiable!**

---

## 📋 DOCUMENTATION MANAGEMENT

### 📖 Before Documentation
1. **READ** entire `docs/` structure
2. **VERIFY** no duplicates exist
3. **MAINTAIN** consistency
4. **ORGANIZE** properly

### 📝 progress.md Management
- **READ** before ANY work
- **UPDATE** with advances
- **TRACK** subagent outputs
- **MAINTAIN** accuracy

---

## 🔧 TECHNICAL IMPLEMENTATION

### 📡 ZMQ Communication
- Compatible with reference UE
- Same format as srsRAN
- Validate with real connections
- Study srsRAN first

### 📚 Layer Architecture
- PHY (Physical Layer)
- MAC (Medium Access Control)  
- RLC (Radio Link Control)
- PDCP (Packet Data Convergence Protocol)
- RRC (Radio Resource Control)
- NGAP (NG Application Protocol)

### ⚡ Performance Requirements
- Rust zero-cost abstractions
- High concurrency design
- Efficient memory management
- Real-time constraints

---

## 📝 IMPORTANT NOTES

1. **Improvements**: Note as comments, implement after validation
2. **Testing**: MUST use quicktest.sh
3. **Logs**: Dated folders for debugging
4. **Configuration**: No hardcoded values
5. **Reference**: ALWAYS check srsRAN first

---

## 🚀 MANDATORY START CHECKLIST

**COMPLETE ALL BEFORE ANY WORK:**
1. ✅ Read ENTIRE CLAUDE.md
2. ✅ ULTRATHINK about task
3. ✅ Read progress.md
4. ✅ Check external_integrations/
5. ✅ Prepare subagent context
6. ✅ Spawn subagent (NOT develop directly)
7. ✅ Validate with quicktest.sh
8. ✅ Update progress.md

**⚠️ INCOMPLETE CHECKLIST = DO NOT START ⚠️**

---

# 🔴🔴🔴 FINAL ENFORCEMENT REMINDERS 🔴🔴🔴

## ⚡ RULE VIOLATIONS = PROJECT FAILURE ⚡
## 🤖 NO SUBAGENTS = PROJECT FAILURE 🤖
## 🧠 NO ULTRATHINKING = PROJECT FAILURE 🧠
## 📋 NO COMPLIANCE = PROJECT FAILURE 📋

### MANDATORY PRACTICES SUMMARY:
1. **ALWAYS ULTRATHINK** - EVERY decision
2. **ALWAYS USE SUBAGENTS** - EVERY task
3. **ALWAYS UPDATE progress.md** - EVERY session
4. **ALWAYS TEST WITH quicktest.sh** - EVERY change
5. **ALWAYS CHECK external_integrations/** - BEFORE coding
6. **ALWAYS WRITE IN ENGLISH** - NO exceptions
7. **ALWAYS USE DOCKER** - ALL compilation/testing
8. **NEVER USE PLACEHOLDERS** - FULL implementation only

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

### 📋 Reference Configuration Files - SACRED AND UNTOUCHABLE!
All configuration files are in `config/` directory:
```
config/
├── documentation/
│   └── 5g_sa_setup.md         # Complete 5G SA docs
├── open5gs/
│   ├── docker-compose.yml     # 5G Core deployment
│   ├── config/amf.yaml        # AMF configuration
│   └── README.md              # Setup guide
├── srsran_gnb/
│   └── gnb_zmq_10mhz.yml     # SACRED gNodeB config - DO NOT MODIFY!
└── srsue/
    └── ue_nr_zmq_10mhz.conf   # SACRED srsUE config - DO NOT MODIFY!
```

⚠️ **CRITICAL**: The 10MHz configurations are PROVEN to achieve RRC connection!
🔒 **NEVER MODIFY THESE FILES** - Adapt the Rust code instead!

### ⚙️ Critical Parameters to Match
1. **SSB**: Every 20 ms
2. **SIB1**: Every 160 ms
3. **CORESET#0**: Index 12
4. **PRACH**: Config Index 0
5. **Cell ID**: 1
6. **DMRS**: Positions defined

### 🚀 Quick Start Commands - USE SACRED CONFIGS ONLY!
```bash
# 1. Start Open5GS
cd config/open5gs
docker-compose up -d

# 2. Reference gNodeB - MUST USE SACRED 10MHz CONFIG!
cd /opt/srsRAN_Project/build
./apps/gnb/gnb -c /workspace/config/srsran_gnb/gnb_zmq_10mhz.yml

# 3. Run srsUE - MUST USE SACRED 10MHz CONFIG!
cd /opt/srsRAN_4G/build
./srsue/src/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf
```

⚠️ **REMEMBER**: These configs are SACRED! They achieve RRC connection!

### 📊 Validation Checklist
- [ ] UE detects cell
- [ ] PRACH successful
- [ ] Registration complete
- [ ] Data plane active

### 🔧 Test Setup
```bash
# ONLY 2 test scripts allowed (see TEST METHODOLOGY section):
./test_srsran.sh    # Test with srsRAN gNodeB (reference)
./test_albor.sh     # Test with Albor gNodeB (our implementation)

# Or use quicktest.sh wrapper:
./quicktest.sh --srsran  # Calls test_srsran.sh
./quicktest.sh           # Calls test_albor.sh (default)
```

### 📡 Technical Parameters
**Physical Layer**:
- PSS/SSS: Subcarriers -31 to 31
- PBCH: 20 RBs around DC
- CORESET#0: 48 RBs from RB 0
- PDCCH: SI-RNTI (0xFFFF)
- DMRS: Amplitude 0.7071

**MAC Layer**:
- SIB1: ~100 bytes
- SI window: 20 ms
- PRACH: frame%16==1, slot 9
- RA window: 10 slots

**Network**:
- Open5GS: 10.53.1.0/24
- AMF: 10.53.1.2:38412
- UPF: 10.53.1.7
- MongoDB: 10.53.1.100
- DN: 10.45.0.0/16

### 📋 Testing Workflow
1. Deploy Open5GS
2. Verify AMF on 38412
3. Start gNodeB
4. Start UE
5. Verify connectivity

### 🚨 Common Issues
- Port conflicts on 2000/2001
- SCTP kernel support
- Network namespaces
- Timing (10s for Open5GS)

---

# 🚨🚨🚨 FINAL WARNING 🚨🚨🚨
## THIS IS NOT OPTIONAL!
## FOLLOW ALL RULES OR FAIL!
## USE SUBAGENTS OR FAIL!
## ULTRATHINK OR FAIL!
## NO EXCEPTIONS!

**REMEMBER: We REPLICATE srsRAN in Rust. Study first, implement second!**