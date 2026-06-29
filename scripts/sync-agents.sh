#!/bin/bash
# 同步 CLAUDE.md 到 AGENTS.md

if [ ! -f "CLAUDE.md" ]; then
    echo "Error: CLAUDE.md not found"
    exit 1
fi

cp CLAUDE.md AGENTS.md
echo "AGENTS.md synced from CLAUDE.md"
