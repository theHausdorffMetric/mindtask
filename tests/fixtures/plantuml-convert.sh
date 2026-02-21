#!/bin/bash

# PlantUML Diagram Converter
# Uses podman to run plantuml container to convert .puml files to svg/png/pdf

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
PLANTUML_IMAGE="docker.io/plantuml/plantuml:latest"
PLANTUML_PDF_IMAGE="docker.io/aplr/plantuml:latest"

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
            echo "Usage: $0 [-f svg|png|pdf] <plantuml-file.puml>"
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
    echo "Usage: $0 [-f svg|png|pdf] <plantuml-file.puml>"
    echo "Example: $0 diagram.puml"
    echo "         $0 -f png diagram.puml"
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

# Select image — PDF requires aplr/plantuml which bundles Batik+FOP JARs
if [ "$FORMAT" = "pdf" ]; then
    IMAGE="$PLANTUML_PDF_IMAGE"
else
    IMAGE="$PLANTUML_IMAGE"
fi

# Create a temporary directory for container I/O
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

# Copy input file to temp directory
cp "$INPUT_FILE" "$TEMP_DIR/$INPUT_BASENAME"

echo -e "${YELLOW}Converting $INPUT_BASENAME to ${FORMAT^^}...${NC}"

# Pull image if needed
echo -e "${YELLOW}Ensuring image is available...${NC}"
if ! podman image exists "$IMAGE"; then
    echo -e "${YELLOW}Pulling $IMAGE${NC}"
    podman pull "$IMAGE"
fi

# Run plantuml container
# Container runs as root, which maps to host UID in rootless podman -- no chmod needed
# --network=none: file conversion needs no network access
# --cap-drop=ALL: no Linux capabilities needed
# --tmpfs /tmp: Java/PlantUML needs a writable /tmp
# :Z relabels the volume for SELinux
podman run --rm \
    --network=none \
    --cap-drop=ALL \
    --security-opt no-new-privileges:true \
    --tmpfs /tmp:rw,noexec,nosuid,size=64m \
    -v "$TEMP_DIR:/data:Z" \
    "$IMAGE" \
    -t${FORMAT} -o "/data" "/data/$INPUT_BASENAME"

# Check if output was created and copy to current directory
if [ -f "$TEMP_DIR/$OUTPUT_FILE" ]; then
    cp "$TEMP_DIR/$OUTPUT_FILE" "./$OUTPUT_FILE"
    chmod 644 "./$OUTPUT_FILE"
    echo -e "${GREEN}✓ Successfully created: $OUTPUT_FILE${NC}"
else
    echo -e "${RED}✗ Failed to create ${FORMAT^^} file${NC}"
    exit 1
fi
