# NCreate Launcher 1.0.0 verification

Release gate is still being evaluated. This document is updated with measured
results; pending checks are not PASS. No v1.0.0 tag or GitHub Release was published.

See [audit and visual mapping](AUDIT.md). Reproducible checks are specified in
AGENTS.md. Local raw evidence lives in `verification/stable-1.0.0/` (ignored by Git).

Current recorded observations: real Linux Mojang Java 8/16/17/21 probes and vanilla
Minecraft 1.16.5/1.17/1.18/1.21.1 launch/stop paths passed before the host restart.
Official NCreate Server 1.0.2 installed and reached NeoForge resource loading, but
Linux OOM-killer terminated two runs. After the user freed RAM, verification was
restarted with the manifest-recommended memory. These earlier runs are not PASS.

Physical Windows, authenticated Microsoft/Ely.by sessions, joining the server,
and a production self-update installation remain unverified.
