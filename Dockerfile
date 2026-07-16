# claude-lite: Arch-based Claude Code image with a prebuilt batch-tools MCP
# server. Nothing is copied from local build context -- every asset is
# fetched over HTTP from the fixed GitHub release URL below at build time,
# so this Dockerfile is the only file anyone needs.
FROM archlinux:latest

LABEL org.opencontainers.image.title="claude-lite"

ARG ASSETS_URL="https://github.com/starhost-app/claude-lite/releases/download/v0.0.1"

RUN pacman -Syu --noconfirm --needed \
        curl git ripgrep base-devel sudo \
    && pacman -Scc --noconfirm

# Claude Code's native installer
RUN curl -fsSL https://claude.ai/install.sh | bash \
    && mv /root/.local/bin/claude /usr/local/bin/claude

RUN curl -fsSL "${ASSETS_URL}/mcp-batch-server" -o /usr/local/bin/mcp-batch-server \
    && chmod +x /usr/local/bin/mcp-batch-server

RUN mkdir -p /root/.claude/skills/container-guide \
    && curl -fsSL "${ASSETS_URL}/claude-settings.json" -o /root/.claude/settings.json \
    && curl -fsSL "${ASSETS_URL}/claude.json" -o /root/.claude.json \
    && curl -fsSL "${ASSETS_URL}/SKILL.md" -o /root/.claude/skills/container-guide/SKILL.md \
    && curl -fsSL "${ASSETS_URL}/entrypoint.sh" -o /entrypoint.sh \
    && chmod +x /entrypoint.sh

WORKDIR /workspace
VOLUME /workspace

ENTRYPOINT ["/entrypoint.sh"]
