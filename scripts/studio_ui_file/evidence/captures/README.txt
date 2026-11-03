h-home-empty: the Recents view of a library with no file, captured from the installed designos 0.7.5 (the build every qa075-dmg capture came from), 1440x900 at scale 1.
Command (hermetic HOME/TMPDIR/DESIGNOS_LIBRARY under the session scratchpad; the library holds only the two seed markers `.seeded` and `.starter-seeded`, so the app seeds nothing and Home draws `empty_state`):
  env -i HOME=<dir> PATH=/usr/bin:/bin TMPDIR=<dir> DESIGNOS_LIBRARY=<lib> /Applications/designos.app/Contents/MacOS/designos --home --size 1440x900 --no-toast --screenshot h-home-empty.png --layout-json h-home-empty.layout.json
Measured in it: title "No files yet" ink x 809-894; body line 1 ink x 667-1033, line 2 ink x 668-868 (left-aligned lines in a block centred on x 852); tile ink x 836-867, y 462-493.
