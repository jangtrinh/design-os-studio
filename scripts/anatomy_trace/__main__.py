"""`python3 scripts/anatomy_trace ...` and `python3 -m anatomy_trace ...` (from scripts/)."""

import sys
from pathlib import Path

# Run as a directory, sys.path[0] is this package itself: swap it for its parent so the modules
# (config, text, svg, ...) are only reachable as anatomy_trace.<module>.
sys.path[0] = str(Path(__file__).resolve().parent.parent)

from anatomy_trace.cli import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
