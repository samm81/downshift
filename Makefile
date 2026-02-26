APP_NAME := Downshift
BIN_NAME := downshift
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
BUNDLE_ID := io.github.downshift
MIN_MACOS := 12.0
DIST_DIR := dist
TAG ?=
TAG_VERSION := $(patsubst v%,%,$(TAG))
RELEASE_SUFFIX := $(if $(TAG),-v$(TAG_VERSION),)
APP_BUNDLE := $(DIST_DIR)/$(APP_NAME).app
DMG_PATH := $(DIST_DIR)/$(APP_NAME)-unsigned$(RELEASE_SUFFIX).dmg
ZIP_PATH := $(DIST_DIR)/$(APP_NAME)-unsigned$(RELEASE_SUFFIX).zip
CHECKSUMS_PATH := $(DIST_DIR)/SHA256SUMS.txt
RUN_RESET := $(filter 1,$(RESET))

.PHONY: all \
	build build-no-telemetry \
	build-debug build-debug-no-telemetry \
	run run-no-telemetry \
	app app-no-telemetry \
	sign-app \
	dmg dmg-no-telemetry \
	zip zip-no-telemetry \
	checksums checksums-no-telemetry \
	check-tag-sync require-telemetry-env package-app \
	release release-no-telemetry clean

all: app

require-telemetry-env:
	@if [ -z "$${DOWNSHIFT_TELEMETRY_ENABLED}" ]; then \
		echo "error: DOWNSHIFT_TELEMETRY_ENABLED is required (set to true for telemetry-enabled builds)"; \
		exit 1; \
	fi
	@if [ -z "$${DOWNSHIFT_BETTERSTACK_LOGS_TOKEN}" ]; then \
		echo "error: DOWNSHIFT_BETTERSTACK_LOGS_TOKEN is required"; \
		exit 1; \
	fi
	@if [ -z "$${DOWNSHIFT_BETTERSTACK_LOGS_HOST}" ]; then \
		echo "error: DOWNSHIFT_BETTERSTACK_LOGS_HOST is required"; \
		exit 1; \
	fi
	@if [ -z "$${DOWNSHIFT_BETTERSTACK_ERRORS_DSN}" ]; then \
		echo "error: DOWNSHIFT_BETTERSTACK_ERRORS_DSN is required"; \
		exit 1; \
	fi
	@if [ -z "$${DOWNSHIFT_BUILD_CHANNEL}" ]; then \
		echo "error: DOWNSHIFT_BUILD_CHANNEL is required"; \
		exit 1; \
	fi
	@if [ "$${DOWNSHIFT_TELEMETRY_ENABLED}" != "true" ]; then \
		echo "error: DOWNSHIFT_TELEMETRY_ENABLED must be 'true' for default telemetry-enabled targets"; \
		exit 1; \
	fi

build: require-telemetry-env
	cargo build --release

build-no-telemetry:
	cargo build --release

build-debug: require-telemetry-env
	cargo build --quiet

build-debug-no-telemetry:
	cargo build --quiet

run: build-debug
ifneq ($(RUN_RESET),)
	@echo "resetting saved downshift settings"
	@rm -f "$$HOME/Library/Application Support/downshift/settings.toml" "$$HOME/.config/downshift/settings.toml"
endif
	./target/debug/$(BIN_NAME)

run-no-telemetry: build-debug-no-telemetry
ifneq ($(RUN_RESET),)
	@echo "resetting saved downshift settings"
	@rm -f "$$HOME/Library/Application Support/downshift/settings.toml" "$$HOME/.config/downshift/settings.toml"
endif
	./target/debug/$(BIN_NAME)

package-app:
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

sign-app:
	codesign --force --deep --sign - "$(APP_BUNDLE)"

app: build package-app sign-app

app-no-telemetry: build-no-telemetry package-app sign-app

zip: app
	rm -f "$(ZIP_PATH)"
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(ZIP_PATH)"

zip-no-telemetry: app-no-telemetry
	rm -f "$(ZIP_PATH)"
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(ZIP_PATH)"

dmg: app
	rm -f "$(DMG_PATH)"
	hdiutil create \
		-volname "$(APP_NAME)" \
		-srcfolder "$(APP_BUNDLE)" \
		-ov -format UDZO \
		"$(DMG_PATH)"

dmg-no-telemetry: app-no-telemetry
	rm -f "$(DMG_PATH)"
	hdiutil create \
		-volname "$(APP_NAME)" \
		-srcfolder "$(APP_BUNDLE)" \
		-ov -format UDZO \
		"$(DMG_PATH)"

checksums: zip dmg
	shasum -a 256 "$(ZIP_PATH)" "$(DMG_PATH)" > "$(CHECKSUMS_PATH)"

checksums-no-telemetry: zip-no-telemetry dmg-no-telemetry
	shasum -a 256 "$(ZIP_PATH)" "$(DMG_PATH)" > "$(CHECKSUMS_PATH)"

check-tag-sync:
	@if [ -n "$(TAG)" ] && [ "$(TAG_VERSION)" != "$(VERSION)" ]; then \
		echo "error: tag $(TAG) does not match cargo version $(VERSION)"; \
		exit 1; \
	fi

release: check-tag-sync checksums

release-no-telemetry: check-tag-sync checksums-no-telemetry

clean:
	rm -rf "$(APP_BUNDLE)"
	rm -f "$(DMG_PATH)" "$(ZIP_PATH)" "$(CHECKSUMS_PATH)"
