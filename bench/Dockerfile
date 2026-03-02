FROM archlinux:latest

RUN pacman -Syu --noconfirm hyperfine fish

# Create user to match host UID paths
RUN useradd -m bench
USER bench
WORKDIR /home/bench

# Copy pre-built binaries
COPY --chown=bench:bench binaries/ /home/bench/.local/bin/
RUN chmod +x /home/bench/.local/bin/*

# Copy benchmark script
COPY --chown=bench:bench docker_bench.sh /home/bench/docker_bench.sh
RUN chmod +x /home/bench/docker_bench.sh

# Data will be mounted at runtime:
#   -v <generated_data>:/home/bench/.claude/projects

ENV PATH="/home/bench/.local/bin:${PATH}"

ENTRYPOINT ["/home/bench/docker_bench.sh"]
