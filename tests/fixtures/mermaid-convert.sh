#!/bin/bash

# Mermaid Diagram Converter
# Uses podman to run mermaid-cli container to convert .mmd files to svg/png/pdf

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
MERMAID_IMAGE="ghcr.io/mermaid-js/mermaid-cli/mermaid-cli:latest"

# Default format
FORMAT="svg"

# Parse options
while [ $# -gt 0 ]; do
    case "$1" in
        -f)
            if [ -z "$2" ]; then
                echo -e "${RED}Error: -f requires a format argument (svg, png, pdf)${NC}"
                exit 1
            fi
            FORMAT="$2"
            shift 2
            ;;
        -*)
            echo -e "${RED}Error: Unknown option '$1'${NC}"
            echo "Usage: $0 [-f svg|png|pdf] <mermaid-file.mmd>"
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

# Validate format
case "$FORMAT" in
    svg|png|pdf) ;;
    *)
        echo -e "${RED}Error: Unsupported format '$FORMAT'. Use svg, png, or pdf${NC}"
        exit 1
        ;;
esac

# Check arguments
if [ $# -eq 0 ]; then
    echo -e "${RED}Error: No input file specified${NC}"
    echo "Usage: $0 [-f svg|png|pdf] <mermaid-file.mmd>"
    echo "Example: $0 diagram.mmd"
    echo "         $0 -f png diagram.mmd"
    exit 1
fi

INPUT_FILE="$1"
INPUT_BASENAME=$(basename "$INPUT_FILE")
INPUT_NAME="${INPUT_BASENAME%.*}"
OUTPUT_FILE="${INPUT_NAME}.${FORMAT}"

# Check if input file exists
if [ ! -f "$INPUT_FILE" ]; then
    echo -e "${RED}Error: File '$INPUT_FILE' not found${NC}"
    exit 1
fi

# Check if podman is installed
if ! command -v podman &> /dev/null; then
    echo -e "${RED}Error: podman is not installed${NC}"
    exit 1
fi

# Create a temporary directory for container I/O
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

# Copy input file to temp directory
cp "$INPUT_FILE" "$TEMP_DIR/$INPUT_BASENAME"

echo -e "${YELLOW}Converting $INPUT_BASENAME to ${FORMAT^^}...${NC}"

# Pull image if needed
echo -e "${YELLOW}Ensuring image is available...${NC}"
if ! podman image exists "$MERMAID_IMAGE"; then
    echo -e "${YELLOW}Pulling $MERMAID_IMAGE${NC}"
    podman pull "$MERMAID_IMAGE" || {
        echo -e "${RED}Failed to pull image. Trying alternative...${NC}"
        MERMAID_IMAGE="docker.io/minlag/mermaid-cli:latest"
        podman pull "$MERMAID_IMAGE"
    }
fi

# Detect the UID/GID the container image runs as (e.g. mermaidcli=1001)
# so --userns=keep-id can map our host UID to the correct container user
CONTAINER_USER=$(podman image inspect "$MERMAID_IMAGE" --format '{{.User}}' 2>/dev/null)
if [ -z "$CONTAINER_USER" ] || [ "$CONTAINER_USER" = "0" ] || [ "$CONTAINER_USER" = "root" ]; then
    # Container runs as root — default rootless mapping is sufficient
    USERNS_ARG=""
else
    # Resolve to numeric UID:GID (handles both numeric and named users)
    CONTAINER_IDS=$(podman run --rm --entrypoint /bin/sh "$MERMAID_IMAGE" -c "echo \$(id -u):\$(id -g)")
    CONTAINER_UID="${CONTAINER_IDS%%:*}"
    CONTAINER_GID="${CONTAINER_IDS##*:}"
    USERNS_ARG="--userns=keep-id:uid=${CONTAINER_UID},gid=${CONTAINER_GID}"
fi

# Build format-specific flags
FORMAT_ARGS=()
if [ "$FORMAT" = "pdf" ]; then
    FORMAT_ARGS+=(--pdfFit)
fi

# Run mermaid-cli container
# --userns=keep-id: maps host UID to the container user so it can read/write
#   the mounted volume without chmod 777
# --network=none: file conversion needs no network access
# --cap-drop=ALL: no Linux capabilities needed
# :Z relabels the volume for SELinux
podman run --rm \
    $USERNS_ARG \
    --network=none \
    --cap-drop=ALL \
    --security-opt no-new-privileges:true \
    -v "$TEMP_DIR:/data:Z" \
    "$MERMAID_IMAGE" \
    -i "/data/$INPUT_BASENAME" \
    -o "/data/$OUTPUT_FILE" \
    "${FORMAT_ARGS[@]}"

# Check if output was created and copy to current directory
if [ -f "$TEMP_DIR/$OUTPUT_FILE" ]; then
    cp "$TEMP_DIR/$OUTPUT_FILE" "./$OUTPUT_FILE"
    chmod 644 "./$OUTPUT_FILE"
    echo -e "${GREEN}✓ Successfully created: $OUTPUT_FILE${NC}"
else
    echo -e "${RED}✗ Failed to create ${FORMAT^^} file${NC}"
    exit 1
fi
