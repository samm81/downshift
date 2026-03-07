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
SIGNED_ZIP_PATH := $(DIST_DIR)/$(APP_NAME)-signed$(RELEASE_SUFFIX).zip
NOTARIZED_DMG_PATH := $(DIST_DIR)/$(APP_NAME)-notarized$(RELEASE_SUFFIX).dmg
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
	require-notarization-env release release-no-telemetry release-notarized clean

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

require-notarization-env:
	@if [ -z "$${MACOS_CERT_P12_B64}" ]; then \
		echo "error: MACOS_CERT_P12_B64 is required"; \
		exit 1; \
	fi
	@if [ -z "$${MACOS_CERT_P12_PASSWORD}" ]; then \
		echo "error: MACOS_CERT_P12_PASSWORD is required"; \
		exit 1; \
	fi
	@if [ -z "$${MACOS_KEYCHAIN_PASSWORD}" ]; then \
		echo "error: MACOS_KEYCHAIN_PASSWORD is required"; \
		exit 1; \
	fi
	@if [ -z "$${MACOS_SIGNING_IDENTITY}" ]; then \
		echo "error: MACOS_SIGNING_IDENTITY is required"; \
		exit 1; \
	fi
	@if [ -z "$${MACOS_NOTARY_APPLE_ID}" ]; then \
		echo "error: MACOS_NOTARY_APPLE_ID is required"; \
		exit 1; \
	fi
	@if [ -z "$${MACOS_NOTARY_APP_PASSWORD}" ]; then \
		echo "error: MACOS_NOTARY_APP_PASSWORD is required"; \
		exit 1; \
	fi
	@if [ -z "$${MACOS_NOTARY_TEAM_ID}" ]; then \
		echo "error: MACOS_NOTARY_TEAM_ID is required"; \
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

release-notarized: check-tag-sync require-telemetry-env require-notarization-env build package-app
	@set -euo pipefail; \
	KEYCHAIN_PATH="$(PWD)/$(DIST_DIR)/downshift-signing.keychain-db"; \
	CERT_PATH="$(PWD)/$(DIST_DIR)/developer-id.p12"; \
	cleanup() { \
		security delete-keychain "$$KEYCHAIN_PATH" >/dev/null 2>&1 || true; \
		rm -f "$$CERT_PATH"; \
	}; \
	trap cleanup EXIT; \
	mkdir -p "$(DIST_DIR)"; \
	printf '%s' "$$MACOS_CERT_P12_B64" | tr -d '\r\n\t ' | openssl base64 -d -A > "$$CERT_PATH"; \
	if [ ! -s "$$CERT_PATH" ]; then \
		echo "error: decoded signing certificate is empty; check MACOS_CERT_P12_B64 secret"; \
		exit 1; \
	fi; \
	if ! openssl pkcs12 -in "$$CERT_PATH" -passin "pass:$$MACOS_CERT_P12_PASSWORD" -noout >/dev/null 2>&1; then \
		echo "error: decoded signing certificate is not a valid PKCS#12 bundle or password is incorrect"; \
		echo "hint: re-export the Developer ID certificate as .p12, base64 it, and update MACOS_CERT_P12_B64 + MACOS_CERT_P12_PASSWORD"; \
		exit 1; \
	fi; \
	security delete-keychain "$$KEYCHAIN_PATH" >/dev/null 2>&1 || true; \
	security create-keychain -p "$$MACOS_KEYCHAIN_PASSWORD" "$$KEYCHAIN_PATH"; \
	security set-keychain-settings -lut 21600 "$$KEYCHAIN_PATH"; \
	security unlock-keychain -p "$$MACOS_KEYCHAIN_PASSWORD" "$$KEYCHAIN_PATH"; \
	security import "$$CERT_PATH" -k "$$KEYCHAIN_PATH" -P "$$MACOS_CERT_P12_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security; \
	security set-key-partition-list -S apple-tool:,apple: -s -k "$$MACOS_KEYCHAIN_PASSWORD" "$$KEYCHAIN_PATH"; \
	find "$(APP_BUNDLE)/Contents" -type d \( -name "*.framework" -o -name "*.app" -o -name "*.xpc" -o -name "*.appex" -o -name "*.bundle" \) -print | \
		awk '{ print length, $$0 }' | sort -rn | cut -d" " -f2- | \
		while IFS= read -r nested; do \
			[ -n "$$nested" ] || continue; \
			codesign --force --options runtime --timestamp --sign "$$MACOS_SIGNING_IDENTITY" --keychain "$$KEYCHAIN_PATH" "$$nested"; \
		done; \
	find "$(APP_BUNDLE)/Contents" -type f -perm -111 -print | \
		while IFS= read -r executable; do \
			codesign --force --options runtime --timestamp --sign "$$MACOS_SIGNING_IDENTITY" --keychain "$$KEYCHAIN_PATH" "$$executable"; \
		done; \
	codesign --force --options runtime --timestamp --sign "$$MACOS_SIGNING_IDENTITY" --keychain "$$KEYCHAIN_PATH" "$(APP_BUNDLE)"; \
	codesign --verify --deep --strict --verbose=2 "$(APP_BUNDLE)"; \
	rm -f "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" "$(CHECKSUMS_PATH)"; \
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(SIGNED_ZIP_PATH)"; \
	hdiutil create -volname "$(APP_NAME)" -srcfolder "$(APP_BUNDLE)" -ov -format UDZO "$(NOTARIZED_DMG_PATH)"; \
	codesign --force --timestamp --sign "$$MACOS_SIGNING_IDENTITY" --keychain "$$KEYCHAIN_PATH" "$(NOTARIZED_DMG_PATH)"; \
	xcrun notarytool submit "$(NOTARIZED_DMG_PATH)" --apple-id "$$MACOS_NOTARY_APPLE_ID" --password "$$MACOS_NOTARY_APP_PASSWORD" --team-id "$$MACOS_NOTARY_TEAM_ID" --wait; \
	xcrun stapler staple -v "$(NOTARIZED_DMG_PATH)"; \
	spctl -a -vv --type open "$(NOTARIZED_DMG_PATH)"; \
	shasum -a 256 "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" > "$(CHECKSUMS_PATH)"

clean:
	rm -rf "$(APP_BUNDLE)"
	rm -f "$(DMG_PATH)" "$(ZIP_PATH)" "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" "$(CHECKSUMS_PATH)"
	rm -f "$(DIST_DIR)/developer-id.p12"
	rm -f "$(DIST_DIR)/downshift-signing.keychain-db" "$(DIST_DIR)/downshift-signing.keychain-db-db"
