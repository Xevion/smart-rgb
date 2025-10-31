set shell := ["powershell"]

default:
    just --list

format:
    @cargo fmt --all

# Run benchmarks with optional arguments
# Examples:
#   just bench                    # Run all benchmarks
#   just bench border_updates     # Run specific benchmark
#   just bench -- --save-baseline my_baseline  # Save baseline
bench *args:
    @cargo bench -p borders-core {{ args }}

# Run tests with custom nextest arguments
# Examples:
#   just test                          # Run all tests
#   just test -p borders-core          # Run tests in specific package
#   just test --test spawn_tests       # Run specific test binary
#   just test -- test_name_pattern     # Filter by test name
#   just test -- --skip pattern        # Skip tests matching pattern
test *args:
    @cargo nextest run --no-fail-fast --workspace --hide-progress-bar --failure-output final {{ args }}

# Run tests matching a pattern (filters by test name)
# Add -p <package> to run in specific package, or --workspace for all packages
test-filter pattern *args:
    @cargo nextest run --no-fail-fast --hide-progress-bar --failure-output final {{ args }} -- {{ pattern }}

# Skip tests matching a pattern
# Add -p <package> to run in specific package, or --workspace for all packages
test-skip pattern *args:
    @cargo nextest run --no-fail-fast --hide-progress-bar --failure-output final {{ args }} -- --skip {{ pattern }}

# Generate coverage reports (HTML, JSON, LCOV)
# Examples:
#   just coverage                 # Generate all formats in coverage/ directory
#   just coverage --open          # Generate and open HTML report in browser
coverage *args:
    @cargo llvm-cov nextest -p borders-core --no-fail-fast {{ args }}
    @if (-not (Test-Path coverage)) { New-Item -ItemType Directory -Path coverage | Out-Null }
    @cargo llvm-cov report --html --output-dir coverage/html
    @cargo llvm-cov report --json --output-path coverage/coverage.json
    @cargo llvm-cov report --lcov --output-path coverage/lcov.info
    @echo "Coverage reports generated in coverage/ directory:"
    @echo "  - HTML: coverage/html/index.html"
    @echo "  - JSON: coverage/coverage.json"
    @echo "  - LCOV: coverage/lcov.info"

# Open coverage HTML report in browser
coverage-open:
    @if (Test-Path coverage/html/index.html) { \
        Invoke-Item coverage/html/index.html; \
    } else { \
        echo "Coverage report not found. Run 'just coverage' first."; \
        exit 1; \
    }

mutants *args:
    cargo mutants -p borders-core --gitignore true --caught --unviable {{ args }}

mutants-next *args:
    cargo mutants -p borders-core --gitignore true --iterate --caught --unviable {{ args }}

check:
    @echo "Running clippy (native)..."
    @cargo clippy --all-targets --all-features --workspace -- -D warnings
    @echo "Running cargo check (native)..."
    @cargo check --all-targets --all-features --workspace
    @echo "Running clippy (wasm32-unknown-unknown)..."
    @cargo clippy --target wasm32-unknown-unknown --all-features -p borders-wasm -- -D warnings
    @echo "Running cargo check (wasm32-unknown-unknown)..."
    @cargo check --target wasm32-unknown-unknown --all-features -p borders-wasm
    @echo "Running cargo machete..."
    @cargo machete --with-metadata
    @echo "All checks passed"

check-ts:
    @just _wasm-build wasm-dev
    @echo "Running frontend checks..."
    @pnpm run -C frontend check

fix:
    @echo "Running cargo fix..."
    cargo fix --all-targets --all-features --workspace --allow-dirty

wasm-dev: wasm-dev-build
    pnpm -C frontend dev:browser --port 1421

# Build WASM with the specified profile (wasm-dev or wasm-release)
_wasm-build profile:
    @$profile = "{{ profile }}"; \
    $wasmFile = "target/wasm32-unknown-unknown/$profile/borders_wasm.wasm"; \
    $pkgJs = "pkg/borders.js"; \
    $pkgWasm = "pkg/borders_bg.wasm"; \
    $frontendPkgJs = "frontend/pkg/borders.js"; \
    $frontendPkgWasm = "frontend/pkg/borders_bg.wasm"; \
    $beforeTime = if (Test-Path $wasmFile) { (Get-Item $wasmFile).LastWriteTime } else { $null }; \
    cargo build -p borders-wasm --profile $profile --target wasm32-unknown-unknown; \
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; \
    $afterTime = if (Test-Path $wasmFile) { (Get-Item $wasmFile).LastWriteTime } else { $null }; \
    $wasRebuilt = ($beforeTime -eq $null) -or ($afterTime -ne $beforeTime); \
    $pkgExists = (Test-Path $pkgJs) -and (Test-Path $pkgWasm); \
    $frontendPkgExists = (Test-Path $frontendPkgJs) -and (Test-Path $frontendPkgWasm); \
    $isRelease = $profile -eq "wasm-release"; \
    $isDebug = $profile -eq "wasm-debug"; \
    $needsFrontendBuild = $isRelease -or $isDebug; \
    if ($wasRebuilt -or -not $pkgExists -or ($needsFrontendBuild -and -not $frontendPkgExists)) { \
        Write-Host "Running wasm-bindgen..."; \
        wasm-bindgen --out-dir pkg --out-name borders --target web $wasmFile; \
        if ($isRelease) { \
            Write-Host "Running wasm-opt -Oz..."; \
            wasm-opt -Oz --enable-bulk-memory --enable-threads --all-features pkg/borders_bg.wasm -o pkg/borders_bg.wasm; \
        } elseif ($isDebug) { \
            Write-Host "Running wasm-opt -O3 with debug info..."; \
            wasm-opt -O3 --debuginfo --enable-bulk-memory --enable-threads --all-features pkg/borders_bg.wasm -o pkg/borders_bg.wasm; \
        }; \
    } else { \
        Write-Host "WASM not rebuilt, skipping wasm-bindgen"; \
    } \
    New-Item -ItemType Directory -Force -Path 'frontend/pkg' | Out-Null; \
    Copy-Item -Recurse -Force 'pkg/*' 'frontend/pkg/'; \
    if ($needsFrontendBuild) { \
        Write-Host "Running frontend build..."; \
        if ($isDebug) { \
            $env:VITE_DEBUG_BUILD = "true"; \
        }; \
        pnpm -C frontend build:browser; \
        if ($isDebug) { \
            Remove-Item env:VITE_DEBUG_BUILD; \
        }; \
    }; \

# Development WASM build, unoptimized
wasm-dev-build:
    @just _wasm-build wasm-dev

# Release WASM build, optimized
wasm-release-build:
    @just _wasm-build wasm-release

wasm-release: wasm-release-build
    @echo "Visit http://localhost:8080 to play"
    caddy file-server --listen :8080 --root frontend/dist/browser/client --browse

# Debug WASM build, optimized with debug symbols
wasm-debug-build:
    @just _wasm-build wasm-debug

wasm-debug: wasm-debug-build
    @echo "Visit http://localhost:8080 to play"
    caddy file-server --listen :8080 --root frontend/dist/browser/client --browse

desktop-release:
    cargo tauri build
    target/release/borders-desktop.exe

desktop-dev:
    cargo tauri build --debug
    target/debug/borders-desktop.exe

dev *args:
    cargo tauri dev {{ args }}

# Run release manager CLI (handles setup specially)
release *args:
    @$firstArg = "{{ args }}".Split()[0]; \
    if ($firstArg -eq "setup") { \
        Write-Host "Installing release manager dependencies..."; \
        pnpm install -C scripts/release-manager; \
    } else { \
        pnpm --silent -C scripts/release-manager exec tsx cli.ts {{ args }}; \
    }
