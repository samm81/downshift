APP_NAME := BreathBall
BIN_NAME := breath-ball
VERSION := 0.1.0
BUNDLE_ID := com.example.breathball
MIN_MACOS := 12.0
DIST_DIR := dist
APP_BUNDLE := $(DIST_DIR)/$(APP_NAME).app
DMG_PATH := $(DIST_DIR)/$(APP_NAME)-unsigned.dmg
ZIP_PATH := $(DIST_DIR)/$(APP_NAME)-unsigned.zip

.PHONY: all release app dmg zip clean

all: app

release:
	cargo build --release

app: release
	rm -rf "$(APP_BUNDLE)"
	mkdir -p "$(APP_BUNDLE)/Contents/MacOS"
	mkdir -p "$(APP_BUNDLE)/Contents/Resources"
	cp "target/release/$(BIN_NAME)" "$(APP_BUNDLE)/Contents/MacOS/$(BIN_NAME)"
	chmod +x "$(APP_BUNDLE)/Contents/MacOS/$(BIN_NAME)"
	printf '%s\n' \
		'<?xml version="1.0" encoding="UTF-8"?>' \
		'<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' \
		'<plist version="1.0">' \
		'  <dict>' \
		'    <key>CFBundleName</key><string>$(APP_NAME)</string>' \
		'    <key>CFBundleDisplayName</key><string>$(APP_NAME)</string>' \
		'    <key>CFBundleIdentifier</key><string>$(BUNDLE_ID)</string>' \
		'    <key>CFBundleVersion</key><string>$(VERSION)</string>' \
		'    <key>CFBundleShortVersionString</key><string>$(VERSION)</string>' \
		'    <key>CFBundleExecutable</key><string>$(BIN_NAME)</string>' \
		'    <key>CFBundlePackageType</key><string>APPL</string>' \
		'    <key>LSMinimumSystemVersion</key><string>$(MIN_MACOS)</string>' \
		'  </dict>' \
		'</plist>' \
		> "$(APP_BUNDLE)/Contents/Info.plist"

zip: app
	rm -f "$(ZIP_PATH)"
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(ZIP_PATH)"

dmg: app
	rm -f "$(DMG_PATH)"
	hdiutil create \
		-volname "$(APP_NAME)" \
		-srcfolder "$(APP_BUNDLE)" \
		-ov -format UDZO \
		"$(DMG_PATH)"

clean:
	rm -rf "$(APP_BUNDLE)"
	rm -f "$(DMG_PATH)" "$(ZIP_PATH)"
