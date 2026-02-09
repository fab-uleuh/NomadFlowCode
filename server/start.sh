#!/bin/bash
# NomadFlow Server Startup Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Starting NomadFlow Server${NC}"

# Check for Python
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Error: Python 3 is required but not installed.${NC}"
    exit 1
fi

# Check for pip
if ! command -v pip3 &> /dev/null; then
    echo -e "${RED}Error: pip3 is required but not installed.${NC}"
    exit 1
fi

# Check for tmux
if ! command -v tmux &> /dev/null; then
    echo -e "${YELLOW}Warning: tmux is not installed.${NC}"
    echo "Install with: brew install tmux (macOS) or apt install tmux (Linux)"
fi

# Check for ttyd
if ! command -v ttyd &> /dev/null; then
    echo -e "${YELLOW}Warning: ttyd is not installed.${NC}"
    echo "Install with: brew install ttyd (macOS) or apt install ttyd (Linux)"
fi

# Create virtual environment if it doesn't exist
if [ ! -d "venv" ]; then
    echo -e "${GREEN}Creating virtual environment...${NC}"
    python3 -m venv venv
fi

# Activate virtual environment
source venv/bin/activate

# Install dependencies
echo -e "${GREEN}Installing dependencies...${NC}"
pip install -q -r requirements.txt

# Run the server
echo -e "${GREEN}Starting server...${NC}"
python -m nomadflow.main
