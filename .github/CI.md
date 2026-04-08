# CI/CD Pipeline Documentation

## Overview

The CSPCL project uses GitHub Actions for continuous integration and deployment. The pipeline automatically runs on every push and pull request to ensure code quality, correctness, and security.

## Pipeline Structure

The CI pipeline consists of 7 jobs that run in parallel (where dependencies allow):

```
┌─────────────────────────────────────────────────────────┐
│                    CI Pipeline                           │
└─────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
    Code Quality      Build & Test        Docker Build
    - Formatting      - Debug build       - Base image
    - ShellCheck      - Release build     - uD3TN image
    - Python black    - Unit tests        - Unibo-BP image
    - clang-format    - Test upload       - Image tests
    - Hadolint
        │                   │                   │
        └───────────────────┼───────────────────┘
                            ▼
                    Docker Integration Tests
                    - ZMQHUB transport
                    - Test execution
                    - Log collection
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
  Documentation         Security            Summary
  - README checks    - Trivy scan        - Status report
  - Link checking    - SARIF upload      - Final result
  - Header docs
```

## Jobs Description

### 1. Code Quality Checks

**Purpose:** Ensure code follows project standards

**Checks:**
- ✅ No trailing whitespace in C/H/SH files
- ✅ Shell scripts pass ShellCheck linting
- ✅ Python code formatted with Black
- ✅ C code formatted according to `.clang-format`
- ✅ Dockerfiles follow best practices (Hadolint)
- 📝 Scan for TODO/FIXME comments (informational)

**Duration:** ~2-3 minutes

**Failure Impact:** Blocks merge (critical)

### 2. Build and Unit Tests

**Purpose:** Verify code compiles and tests pass

**Matrix Strategy:**
- Debug build (with debugging symbols)
- Release build (optimized)

**Steps:**
1. Install build dependencies
2. Cache libcsp v1.6 (speeds up builds)
3. Build libcsp if not cached
4. Configure CSPCL with CMake
5. Build CSPCL library
6. Run CTest unit tests
7. Upload test results as artifacts

**Duration:** ~5-8 minutes (first run), ~3-4 minutes (cached)

**Failure Impact:** Blocks merge (critical)

### 3. Docker Build Test

**Purpose:** Ensure Docker images build successfully

**Images Built:**
- `cspcl-base` - Base image with libcsp + CSPCL
- `cspcl-ud3tn` - uD3TN integration
- `cspcl-unibo` - Unibo-BP integration (may fail without binaries)

**Optimizations:**
- Uses GitHub Actions cache for layer caching
- Builds with Docker Buildx for multi-platform support

**Duration:** ~10-15 minutes (first run), ~3-5 minutes (cached)

**Failure Impact:** Blocks merge (critical for base/ud3tn)

### 4. Docker Integration Tests

**Purpose:** Run end-to-end integration tests

**Tests:**
- uD3TN basic: Two-node bundle transfer
- Unibo-BP basic: Two-node bundle transfer (if available)
- Cross-integration: uD3TN ↔ Unibo-BP (if available)

**Transport:** ZMQHUB (virtual, works in CI)

**Timeout:** 5 minutes per test suite

**Artifacts:** Container logs uploaded on failure

**Duration:** ~5-10 minutes

**Failure Impact:** Warning (some tests may not run without full setup)

### 5. Documentation Check

**Purpose:** Verify documentation is complete and valid

**Checks:**
- Required README files exist
- Markdown links are not broken (external links checked)
- C headers have documentation comments

**Duration:** ~1-2 minutes

**Failure Impact:** Blocks merge (critical)

### 6. Security Scanning

**Purpose:** Identify security vulnerabilities

**Tools:**
- Trivy - Scans for CVEs in dependencies and code

**Output:** SARIF format uploaded to GitHub Security tab

**Duration:** ~2-3 minutes

**Failure Impact:** Warning (doesn't block merge but should be reviewed)

### 7. CI Summary

**Purpose:** Aggregate results and provide overview

**Actions:**
- Creates summary table in job output
- Fails if critical jobs failed
- Provides quick status overview

**Duration:** <1 minute

## Running CI Locally

### Code Quality Checks

```bash
# Format all code
./tools/format-code.sh

# Check shell scripts
find . -name "*.sh" -not -path "./build/*" | xargs shellcheck

# Check Python
pip install black
black tools/ --check

# Check C formatting
find src/ tests/ -name "*.c" -o -name "*.h" | xargs clang-format --dry-run --Werror
```

### Build and Test

```bash
# Build libcsp
cd /tmp
git clone --branch v1.6 --depth 1 https://github.com/libcsp/libcsp.git
cd libcsp
python3 waf configure --enable-can-socketcan --enable-if-zmqhub --with-os=posix
python3 waf build

# Build CSPCL
cd /path/to/cspcl
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Debug -DCSPCL_USE_SYSTEM_CSP=ON -DCSP_REPO_DIR=/tmp/libcsp
make -j$(nproc)

# Run tests
ctest --output-on-failure --verbose
```

### Docker Tests

```bash
# Build base image
docker build -t cspcl-base:latest -f docker/base/Dockerfile .

# Run integration tests
cd tests/interop
./run-tests.sh --transport zmqhub
```

## CI Configuration Files

### Main Pipeline

**File:** `.github/workflows/ci.yml`

Main CI configuration with all jobs defined.

### Code Formatting

**File:** `.clang-format`

C/C++ code formatting rules based on LLVM style:
- 4-space indentation
- 100 column limit
- Linux brace style
- Right pointer alignment

### Markdown Link Checking

**File:** `.github/markdown-link-check-config.json`

Configuration for markdown link checker:
- Ignores known documentation sites
- 20-second timeout per link
- Retries on 429 (rate limiting)

## GitHub Actions Cache

The pipeline uses caching to speed up builds:

### libcsp Cache
- **Key:** `${{ runner.os }}-libcsp-v1.6-${{ hashFiles('.github/workflows/ci.yml') }}`
- **Path:** `/tmp/libcsp`
- **Invalidation:** Changes to CI workflow file

### Docker Layer Cache
- **Type:** GitHub Actions cache
- **Mode:** Max (caches all layers)
- **Benefit:** 5-10x faster Docker builds

## Artifacts

CI generates artifacts that can be downloaded:

### Test Results
- **Job:** Build and Unit Tests
- **Files:** `build/Testing/`
- **Retention:** 30 days
- **Use:** Detailed test output and timing

### Integration Test Logs
- **Job:** Docker Integration Tests  
- **Files:** Container logs, docker-compose output
- **Retention:** 30 days
- **Use:** Debugging failed integration tests

## Badge Status

Add to README.md:

```markdown
[![CI](https://github.com/DTN-MTP/cspcl/actions/workflows/ci.yml/badge.svg)](https://github.com/DTN-MTP/cspcl/actions/workflows/ci.yml)
```

## Troubleshooting CI Failures

### Code Quality Failure: Trailing Whitespace

**Fix:**
```bash
# Remove trailing whitespace
find . -type f \( -name "*.c" -o -name "*.h" -o -name "*.sh" \) \
  -not -path "./build/*" -exec sed -i 's/[[:space:]]*$//' {} \;
```

### Code Quality Failure: C Formatting

**Fix:**
```bash
# Auto-format all C code
./tools/format-code.sh
# Or manually:
find src/ tests/ -name "*.c" -o -name "*.h" | xargs clang-format -i
```

### Code Quality Failure: ShellCheck

**Fix:** Address specific ShellCheck warnings
```bash
shellcheck path/to/script.sh
# Common fixes:
# - Quote variables: "$var" instead of $var
# - Check exit codes: if ! cmd; then ...
# - Use [[ ]] instead of [ ] for conditions
```

### Build Failure: libcsp Not Found

**Cause:** libcsp cache corrupted or version mismatch

**Fix:** Clear cache and rebuild
- Go to Actions → Caches
- Delete libcsp cache
- Re-run workflow

### Docker Build Failure: Context Too Large

**Cause:** Build context includes large files

**Fix:** Add to `.dockerignore`:
```
build/
.git/
*.o
*.a
```

### Integration Test Timeout

**Cause:** Services didn't start or test hung

**Fix:**
1. Check container logs in artifacts
2. Increase timeout in workflow
3. Run locally: `./run-tests.sh --interactive`

## Performance Optimization

### Reduce CI Time

1. **Enable libcsp cache** (already done)
2. **Enable Docker layer cache** (already done)
3. **Run jobs in parallel** (already done)
4. **Use self-hosted runners** (optional, for private repos)

### Current Timings

| Job | Cached | Uncached |
|-----|--------|----------|
| Code Quality | 2 min | 2 min |
| Build & Test | 3 min | 8 min |
| Docker Build | 4 min | 15 min |
| Integration Tests | 6 min | 10 min |
| Documentation | 1 min | 1 min |
| Security | 2 min | 3 min |
| **Total** | **~10 min** | **~25 min** |

## Contributing to CI

### Adding a New Check

1. Edit `.github/workflows/ci.yml`
2. Add step to appropriate job or create new job
3. Test locally first
4. Document in this file

### Adding a New Test

1. Create test script in `tests/interop/`
2. Add to `run-tests.sh`
3. Update Docker Integration Tests job
4. Document in `tests/interop/README.md`

### Modifying Docker Images

1. Update Dockerfile
2. Test build locally
3. CI will automatically build and test
4. Check Docker Build job for errors

## Security Considerations

### Secrets

Never commit secrets to the repository. Use GitHub Secrets for:
- Docker registry credentials
- API tokens
- Deploy keys

### Dependency Scanning

Trivy scans for known vulnerabilities. Review findings:
- Go to Security → Code scanning alerts
- Address critical and high severity issues
- Update dependencies as needed

### SARIF Reports

Security reports are uploaded in SARIF format:
- View in Security tab
- Integrate with third-party tools
- Track over time

## Further Reading

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Docker Build Actions](https://github.com/docker/build-push-action)
- [Trivy Scanner](https://github.com/aquasecurity/trivy)
- [ShellCheck](https://www.shellcheck.net/)
- [clang-format](https://clang.llvm.org/docs/ClangFormat.html)
