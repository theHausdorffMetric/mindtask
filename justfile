set quiet

fixture := "tests/fixtures/sample.json"
out_dir := "tests/fixtures/output"

# List available recipes
default:
    just --list

# Build the project
build:
    cargo build

# Run all tests
test:
    cargo test

# Check SKILL.md documents every CLI command and matches the crate version
test-skill-doc:
    cargo test --test skill_doc_sync

# Generate the sample JSON fixture from scratch
create-fixture: build
    bash tests/fixtures/generate.sh

# Render all PlantUML diagrams from the sample fixture
render: build
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p {{ out_dir }}
    cp {{ fixture }} {{ out_dir }}/.mindtask.json
    cd {{ out_dir }}
    BIN="{{ justfile_directory() }}/target/debug/mindtask"
    "$BIN" export plantuml tree   > tree.puml
    "$BIN" export plantuml tree 1 > tree-backend.puml
    "$BIN" export plantuml dag    > dag.puml
    "$BIN" export plantuml dag 1  > dag-from-1.puml
    "$BIN" export plantuml gantt  > gantt.puml
    "$BIN" export plantuml wbs    > wbs.puml
    "$BIN" export plantuml wbs 1  > wbs-backend.puml
    rm .mindtask.json
    echo "Wrote PlantUML files to {{ out_dir }}/"
    ls -1 *.puml

# Regenerate fixture then render all diagrams
create-and-render: create-fixture render

# Convert all .puml files to SVG via podman (requires podman)
render-svg: render
    #!/usr/bin/env bash
    set -euo pipefail
    CONVERT="{{ justfile_directory() }}/tests/fixtures/plantuml-convert.sh"
    cd {{ out_dir }}
    for f in *.puml; do
        "$CONVERT" "$f"
    done

# Remove generated output files
clean-output:
    rm -rf {{ out_dir }}
