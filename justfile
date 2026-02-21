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
    rm .mindtask.json
    echo "Wrote PlantUML files to {{ out_dir }}/"
    ls -1 *.puml

# Regenerate fixture then render all diagrams
create-and-render: create-fixture render

# Remove generated output files
clean-output:
    rm -rf {{ out_dir }}
