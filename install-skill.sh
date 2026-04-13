#!/bin/bash
# Install the vex skill for Claude Code (global — available in all projects)
set -e

SKILL_DIR="$HOME/.claude/skills/vex"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$SKILL_DIR"
cp "$SCRIPT_DIR/.claude/skills/vex/SKILL.md" "$SKILL_DIR/SKILL.md"

echo "Installed vex skill to $SKILL_DIR"
echo "Usage: /vex \"your search query\" in any Claude Code session"
