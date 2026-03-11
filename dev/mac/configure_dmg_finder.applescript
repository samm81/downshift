on run argv
  set mountPath to item 1 of argv
  set appName to item 2 of argv
  set backgroundName to item 3 of argv
  set backgroundAlias to POSIX file (mountPath & "/.background/" & backgroundName) as alias

  tell application "Finder"
    tell disk (POSIX file mountPath as alias)
      open
      delay 1

      set current view of container window to icon view
      set toolbar visible of container window to false
      set statusbar visible of container window to false
      set bounds of container window to {160, 110, 840, 600}

      set viewOptions to the icon view options of container window
      set arrangement of viewOptions to not arranged
      set icon size of viewOptions to 128
      set text size of viewOptions to 13
      set background picture of viewOptions to backgroundAlias

      set position of item appName of container window to {170, 226}
      set position of item "Applications" of container window to {534, 238}

      close
      open
      update without registering applications
      delay 2
    end tell
  end tell
end run
