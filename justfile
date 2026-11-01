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

# Render the fixture's concept tree and task DAG to SVG via Graphviz (requires dot)
render-dot: build
    #!/usr/bin/env bash
    set -euo pipefail
    command -v dot >/dev/null || { echo "missing required tool: dot (graphviz)"; exit 1; }
    mkdir -p {{ out_dir }}
    BIN="{{ justfile_directory() }}/target/debug/mindtask"
    for d in tree dag; do
        "$BIN" -f {{ fixture }} export dot "$d" > "{{ out_dir }}/$d.dot"
        dot -Tsvg "{{ out_dir }}/$d.dot" -o "{{ out_dir }}/$d.svg"
    done
    echo "Wrote tree.svg and dag.svg to {{ out_dir }}/"

# --- project reports -----------------------------------------------------------
# PDF and HTML are produced outside the binary from its JSON and Markdown
# output (docs/OUTPUT-FORMATS-PLAN.md): Typst reads the JSON, pandoc converts
# the Markdown, Graphviz draws the diagrams. Defaults render the fixture; pass
# a project file to render a real one, e.g.
#   just states=all report-pdf ~/dev/ideas/.mindtask.json "ideas tracker"
# Output lands in `out` (gitignored).

out := "build"
# Task states to include: todo,in_progress (the listings' default) or all
states := "todo,in_progress"
# How much of each description the PDF shows: none, short, full
desc := "short"

# Write report.json + schedule.json for FILE into the build directory
report-json FILE=fixture: build
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{ out }}"
    BIN="{{ justfile_directory() }}/target/debug/mindtask"
    "$BIN" -f "{{ FILE }}" report --format json --state "{{ states }}" > "{{ out }}/report.json"
    "$BIN" -f "{{ FILE }}" schedule --format json > "{{ out }}/schedule.json"
    echo "→ {{ out }}/report.json, {{ out }}/schedule.json"

# Write the Markdown report for FILE (descriptions as `desc`: none, short, full)
report-md FILE=fixture: build
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{ out }}"
    BIN="{{ justfile_directory() }}/target/debug/mindtask"
    case "{{ desc }}" in
        none) flag="" ;;
        short) flag="-d=short" ;;
        full) flag="-d" ;;
        *) echo "desc must be none, short, or full"; exit 1 ;;
    esac
    "$BIN" -f "{{ FILE }}" report --format md --state "{{ states }}" $flag > "{{ out }}/report.md"
    echo "→ {{ out }}/report.md"

# Render the concept tree and task DAG of FILE to SVG (requires dot)
report-svg FILE=fixture: build
    #!/usr/bin/env bash
    set -euo pipefail
    command -v dot >/dev/null || { echo "missing required tool: dot (graphviz)"; exit 1; }
    mkdir -p "{{ out }}"
    BIN="{{ justfile_directory() }}/target/debug/mindtask"
    for d in tree dag; do
        "$BIN" -f "{{ FILE }}" export dot "$d" | dot -Tsvg -o "{{ out }}/$d.svg"
    done
    echo "→ {{ out }}/tree.svg, {{ out }}/dag.svg"

# PDF report of FILE via Typst: tree, grouped tasks, schedule, both diagrams
report-pdf FILE=fixture TITLE="Project report": (report-json FILE) (report-svg FILE)
    #!/usr/bin/env bash
    set -euo pipefail
    command -v typst >/dev/null || { echo "missing required tool: typst"; exit 1; }
    cp "{{ justfile_directory() }}/templates/report.typ" "{{ out }}/report.typ"
    typst compile --input title="{{ TITLE }}" --input desc="{{ desc }}" \
        --input diagrams=tree,dag "{{ out }}/report.typ" "{{ out }}/report.pdf"
    echo "→ {{ out }}/report.pdf"

# Standalone HTML page of FILE via pandoc from the Markdown report
report-html FILE=fixture TITLE="Project report": (report-md FILE)
    #!/usr/bin/env bash
    set -euo pipefail
    command -v pandoc >/dev/null || { echo "missing required tool: pandoc"; exit 1; }
    pandoc "{{ out }}/report.md" --standalone --embed-resources \
        --css "{{ justfile_directory() }}/templates/report.css" \
        --metadata pagetitle="{{ TITLE }}" -o "{{ out }}/report.html"
    echo "→ {{ out }}/report.html"

# Remove the report build directory
clean-report:
    rm -rf "{{ out }}"
