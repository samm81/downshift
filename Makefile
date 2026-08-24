APP_NAME := Downshift
BIN_NAME := downshift
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
BUNDLE_ID := app.getdownshift
MIN_MACOS := 13.0
MACOS_TARGET ?= $(shell rustc -vV 2>/dev/null | sed -n 's/^host: //p')
WINDOWS_VM_ROOT ?= ../downshift-vm
PAGES_DIR := docs
PAGES_PREVIEW_HOST ?= 127.0.0.1
PAGES_PREVIEW_BIND ?= 0.0.0.0
PAGES_PREVIEW_PORT ?= 4173
DIST_DIR := dist
TAG ?=
TAG_VERSION := $(patsubst v%,%,$(TAG))
RELEASE_SUFFIX := $(if $(TAG),-v$(TAG_VERSION),)
APP_BUNDLE := $(DIST_DIR)/$(APP_NAME).app
MACOS_RELEASE_BINARY := target/$(MACOS_TARGET)/release/$(BIN_NAME)
DMG_STAGING_DIR := $(DIST_DIR)/$(APP_NAME)-dmg
TEMP_DMG_PATH := $(DIST_DIR)/$(APP_NAME)-temp.dmg
DMG_MOUNT_DIR := $(DIST_DIR)/dmg-mount
DMG_BACKGROUND_NAME := background.png
DMG_BACKGROUND_SOURCE := $(DIST_DIR)/$(DMG_BACKGROUND_NAME)
DMG_BACKGROUND_DIR := .background
DMG_WINDOW_SCRIPT := dev/mac/configure_dmg_finder.applescript
DMG_BACKGROUND_SCRIPT := dev/mac/render_dmg_background.swift
APP_ICON_NAME := Downshift.icns
APP_ICONSET_DIR := $(DIST_DIR)/downshift.iconset
APP_ICON_SOURCE := $(DIST_DIR)/$(APP_ICON_NAME)
APP_ICON_SCRIPT := dev/mac/render_app_icon.swift
APP_ICON_BASE_SOURCE := docs/assets/icon.png
DMG_PATH := $(DIST_DIR)/$(APP_NAME)-unsigned$(RELEASE_SUFFIX).dmg
ZIP_PATH := $(DIST_DIR)/$(APP_NAME)-unsigned$(RELEASE_SUFFIX).zip
CHECKSUMS_PATH := $(DIST_DIR)/SHA256SUMS.txt
SIGNED_ZIP_PATH := $(DIST_DIR)/$(APP_NAME)-signed$(RELEASE_SUFFIX).zip
NOTARIZED_DMG_PATH := $(DIST_DIR)/$(APP_NAME)-notarized$(RELEASE_SUFFIX).dmg
RUN_RESET := $(filter 1,$(RESET))

.PHONY: all \
	build build-no-telemetry \
	build-macos build-macos-no-telemetry \
	verify-windows build-windows-installer smoke-windows \
	build-debug build-debug-no-telemetry \
	run run-no-telemetry \
	smoke-windows-vm \
	pages-preview pages-check pages-smoke pages-release-manifest \
	app app-no-telemetry \
	verify-rust verify-release \
	sign-app \
	generate-app-icon \
	generate-dmg-background \
	stage-dmg-contents \
	dmg dmg-no-telemetry \
	zip zip-no-telemetry \
	release-pre-notarize \
	staple-notarized-dmg \
	write-release-checksums \
	checksums checksums-no-telemetry \
	verify-notarized-dmg \
	check-tag-sync require-macos-target require-telemetry-env package-app \
	require-notarization-env release release-no-telemetry release-notarized clean

all: app

verify-rust:
	cargo fmt --check
	cargo test
	cargo clippy --all-targets -- -D warnings

verify-release: verify-rust
	npm run check

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

require-macos-target:
	@if [ -z "$(MACOS_TARGET)" ]; then \
		echo "error: MACOS_TARGET is required for macOS packaging (for example MACOS_TARGET=aarch64-apple-darwin)"; \
		exit 1; \
	fi

build-macos: require-macos-target require-telemetry-env
	cargo build --release --target "$(MACOS_TARGET)"

build-macos-no-telemetry: require-macos-target
	cargo build --release --target "$(MACOS_TARGET)"

verify-windows:
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/fast-check.ps1 -Release

build-windows-installer:
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/build-installer.ps1

smoke-windows: build-windows-installer
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/smoke-installer.ps1 -InstallerPath "$(DIST_DIR)/windows/$(APP_NAME)-Setup-$(VERSION).exe"

build-debug: require-telemetry-env
	cargo build --quiet

build-debug-no-telemetry:
	cargo build --quiet

smoke-windows-vm:
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/smoke-vm.ps1 -VmRoot "$(WINDOWS_VM_ROOT)"

pages-preview:
	@command -v python3 >/dev/null 2>&1 || { echo "error: python3 is required for the local GitHub Pages preview"; exit 1; }
	@echo "GitHub Pages preview: http://$(PAGES_PREVIEW_HOST):$(PAGES_PREVIEW_PORT)/"
	@echo "Serving $(PWD)/$(PAGES_DIR); press Ctrl-C to stop."
	python3 -m http.server "$(PAGES_PREVIEW_PORT)" --bind "$(PAGES_PREVIEW_BIND)" --directory "$(PAGES_DIR)"

pages-check:
	node dev/pages/release-manifest.mjs validate docs/release.json

pages-smoke: pages-check
	npm run smoke:pages

pages-release-manifest:
	@if [ -z "$(TAG)" ]; then \
		echo "error: TAG is required (for example TAG=v0.2.0)"; \
		exit 1; \
	fi
	@if [ "$(PUSH)" = "1" ]; then \
		bash dev/pages/update-release-manifest.bash "$(TAG)" --commit --push; \
	else \
		bash dev/pages/update-release-manifest.bash "$(TAG)"; \
	fi

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

package-app: generate-app-icon
	rm -rf "$(APP_BUNDLE)"
	mkdir -p "$(APP_BUNDLE)/Contents/MacOS"
	mkdir -p "$(APP_BUNDLE)/Contents/Resources"
	cp "$(MACOS_RELEASE_BINARY)" "$(APP_BUNDLE)/Contents/MacOS/$(BIN_NAME)"
	chmod +x "$(APP_BUNDLE)/Contents/MacOS/$(BIN_NAME)"
	cp "$(APP_ICON_SOURCE)" "$(APP_BUNDLE)/Contents/Resources/$(APP_ICON_NAME)"
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
		'    <key>CFBundleIconFile</key><string>$(APP_ICON_NAME)</string>' \
		'    <key>CFBundlePackageType</key><string>APPL</string>' \
		'    <key>LSMinimumSystemVersion</key><string>$(MIN_MACOS)</string>' \
		'  </dict>' \
		'</plist>' \
		> "$(APP_BUNDLE)/Contents/Info.plist"

sign-app:
	codesign --force --deep --sign - "$(APP_BUNDLE)"

generate-app-icon:
	rm -rf "$(APP_ICONSET_DIR)"
	swift "$(APP_ICON_SCRIPT)" "$(APP_ICON_BASE_SOURCE)" "$(APP_ICONSET_DIR)"
	rm -f "$(APP_ICON_SOURCE)"
	iconutil -c icns "$(APP_ICONSET_DIR)" -o "$(APP_ICON_SOURCE)"

generate-dmg-background:
	swift "$(DMG_BACKGROUND_SCRIPT)" "$(DMG_BACKGROUND_SOURCE)"

stage-dmg-contents: package-app
	rm -rf "$(DMG_STAGING_DIR)"
	mkdir -p "$(DMG_STAGING_DIR)"
	cp -R "$(APP_BUNDLE)" "$(DMG_STAGING_DIR)/$(APP_NAME).app"
	mkdir -p "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)"
	cp "$(DMG_BACKGROUND_SOURCE)" "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)"

app: build-macos package-app sign-app

app-no-telemetry: build-macos-no-telemetry package-app sign-app

zip: app
	rm -f "$(ZIP_PATH)"
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(ZIP_PATH)"

zip-no-telemetry: app-no-telemetry
	rm -f "$(ZIP_PATH)"
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(ZIP_PATH)"

dmg: app generate-dmg-background stage-dmg-contents
	@set -euo pipefail; \
	hdiutil detach "$(PWD)/$(DMG_MOUNT_DIR)" >/dev/null 2>&1 || true; \
	rm -rf "$(DMG_MOUNT_DIR)"; \
	rm -f "$(TEMP_DMG_PATH)" "$(DMG_PATH)"; \
	SIZE_MB="$$(du -sm "$(DMG_STAGING_DIR)" | awk '{print $$1 + 32}')"; \
	hdiutil create -size "$${SIZE_MB}m" -fs HFS+ -volname "$(APP_NAME)" -ov "$(TEMP_DMG_PATH)"; \
	mkdir -p "$(DMG_MOUNT_DIR)"; \
	hdiutil attach -nobrowse -readwrite -mountpoint "$(PWD)/$(DMG_MOUNT_DIR)" "$(TEMP_DMG_PATH)" >/dev/null; \
	cp -R "$(DMG_STAGING_DIR)/." "$(DMG_MOUNT_DIR)"; \
	ln -s /Applications "$(DMG_MOUNT_DIR)/Applications"; \
	SetFile -a V "$(DMG_MOUNT_DIR)/$(DMG_BACKGROUND_DIR)"; \
	osascript "$(DMG_WINDOW_SCRIPT)" "$(PWD)/$(DMG_MOUNT_DIR)" "$(APP_NAME).app" "$(DMG_BACKGROUND_NAME)"; \
	hdiutil detach "$(PWD)/$(DMG_MOUNT_DIR)" >/dev/null; \
	rm -rf "$(DMG_MOUNT_DIR)"; \
	hdiutil convert "$(TEMP_DMG_PATH)" -ov -format UDZO -o "$(DMG_PATH)"; \
	rm -f "$(TEMP_DMG_PATH)"

dmg-no-telemetry: app-no-telemetry generate-dmg-background stage-dmg-contents
	@set -euo pipefail; \
	hdiutil detach "$(PWD)/$(DMG_MOUNT_DIR)" >/dev/null 2>&1 || true; \
	rm -rf "$(DMG_MOUNT_DIR)"; \
	rm -f "$(TEMP_DMG_PATH)" "$(DMG_PATH)"; \
	SIZE_MB="$$(du -sm "$(DMG_STAGING_DIR)" | awk '{print $$1 + 32}')"; \
	hdiutil create -size "$${SIZE_MB}m" -fs HFS+ -volname "$(APP_NAME)" -ov "$(TEMP_DMG_PATH)"; \
	mkdir -p "$(DMG_MOUNT_DIR)"; \
	hdiutil attach -nobrowse -readwrite -mountpoint "$(PWD)/$(DMG_MOUNT_DIR)" "$(TEMP_DMG_PATH)" >/dev/null; \
	cp -R "$(DMG_STAGING_DIR)/." "$(DMG_MOUNT_DIR)"; \
	ln -s /Applications "$(DMG_MOUNT_DIR)/Applications"; \
	SetFile -a V "$(DMG_MOUNT_DIR)/$(DMG_BACKGROUND_DIR)"; \
	osascript "$(DMG_WINDOW_SCRIPT)" "$(PWD)/$(DMG_MOUNT_DIR)" "$(APP_NAME).app" "$(DMG_BACKGROUND_NAME)"; \
	hdiutil detach "$(PWD)/$(DMG_MOUNT_DIR)" >/dev/null; \
	rm -rf "$(DMG_MOUNT_DIR)"; \
	hdiutil convert "$(TEMP_DMG_PATH)" -ov -format UDZO -o "$(DMG_PATH)"; \
	rm -f "$(TEMP_DMG_PATH)"

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

# Newer macOS runner images ship OpenSSL 3, which needs the legacy provider for older .p12 bundles.
release-pre-notarize: check-tag-sync require-telemetry-env require-notarization-env build-macos package-app generate-dmg-background
	@set -euo pipefail; \
	KEYCHAIN_PATH="$(PWD)/$(DIST_DIR)/downshift-signing.keychain-db"; \
	CERT_PATH="$(PWD)/$(DIST_DIR)/developer-id.p12"; \
	PKCS12_ERROR_PATH="$(DIST_DIR)/pkcs12-validation.error"; \
	cleanup() { \
		security delete-keychain "$$KEYCHAIN_PATH" >/dev/null 2>&1 || true; \
		rm -f "$$CERT_PATH" "$$PKCS12_ERROR_PATH"; \
	}; \
	trap cleanup EXIT; \
	mkdir -p "$(DIST_DIR)"; \
	printf '%s' "$$MACOS_CERT_P12_B64" | tr -d '\r\n\t ' | openssl base64 -d -A > "$$CERT_PATH"; \
	if [ ! -s "$$CERT_PATH" ]; then \
		echo "error: decoded signing certificate is empty; check MACOS_CERT_P12_B64 secret"; \
		exit 1; \
	fi; \
	if ! openssl pkcs12 -in "$$CERT_PATH" -passin "pass:$$MACOS_CERT_P12_PASSWORD" -noout >/dev/null 2>"$$PKCS12_ERROR_PATH"; then \
		if ! openssl pkcs12 -legacy -in "$$CERT_PATH" -passin "pass:$$MACOS_CERT_P12_PASSWORD" -noout >/dev/null 2>"$$PKCS12_ERROR_PATH"; then \
			echo "error: decoded signing certificate is not a valid PKCS#12 bundle or password is incorrect"; \
			echo "hint: re-export the Developer ID certificate as .p12, base64 it, and update MACOS_CERT_P12_B64 + MACOS_CERT_P12_PASSWORD"; \
			echo "OpenSSL: $$(openssl version)"; \
			ERROR_TEXT="$$(tr '\n' ' ' < "$$PKCS12_ERROR_PATH")"; \
			if [ -n "$$ERROR_TEXT" ]; then echo "OpenSSL reported: $$ERROR_TEXT"; fi; \
			exit 1; \
		fi; \
		echo "validated PKCS#12 bundle with OpenSSL legacy provider"; \
	fi; \
	rm -f "$$PKCS12_ERROR_PATH"; \
	security delete-keychain "$$KEYCHAIN_PATH" >/dev/null 2>&1 || true; \
	security create-keychain -p "$$MACOS_KEYCHAIN_PASSWORD" "$$KEYCHAIN_PATH"; \
	security set-keychain-settings -lut 21600 "$$KEYCHAIN_PATH"; \
	security list-keychains -d user -s "$$KEYCHAIN_PATH"; \
	security default-keychain -s "$$KEYCHAIN_PATH"; \
	security unlock-keychain -p "$$MACOS_KEYCHAIN_PASSWORD" "$$KEYCHAIN_PATH"; \
	security import "$$CERT_PATH" -k "$$KEYCHAIN_PATH" -P "$$MACOS_CERT_P12_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security; \
	IDENTITY_SHA="$$(security find-identity -v -p codesigning "$$KEYCHAIN_PATH" | awk 'match($$0,/([0-9A-F]{40})/){print substr($$0,RSTART,RLENGTH); exit}')"; \
	if [ -z "$$IDENTITY_SHA" ]; then \
		echo "error: no codesigning identity found after certificate import"; \
		exit 1; \
	fi; \
	if ! security set-key-partition-list -S apple-tool:,apple: -s -k "$$MACOS_KEYCHAIN_PASSWORD" "$$KEYCHAIN_PATH" >/dev/null 2>&1; then \
		security set-key-partition-list -S apple-tool:,apple: -s -k "$$MACOS_KEYCHAIN_PASSWORD" -Z "$$IDENTITY_SHA" "$$KEYCHAIN_PATH"; \
	fi; \
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
	rm -rf "$(DMG_STAGING_DIR)"; \
	mkdir -p "$(DMG_STAGING_DIR)"; \
	cp -R "$(APP_BUNDLE)" "$(DMG_STAGING_DIR)/$(APP_NAME).app"; \
	mkdir -p "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)"; \
	cp "$(DMG_BACKGROUND_SOURCE)" "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)"; \
	rm -f "$(TEMP_DMG_PATH)"; \
	hdiutil detach "$(PWD)/$(DMG_MOUNT_DIR)" >/dev/null 2>&1 || true; \
	rm -rf "$(DMG_MOUNT_DIR)"; \
	SIZE_MB="$$(du -sm "$(DMG_STAGING_DIR)" | awk '{print $$1 + 32}')"; \
	hdiutil create -size "$${SIZE_MB}m" -fs HFS+ -volname "$(APP_NAME)" -ov "$(TEMP_DMG_PATH)"; \
	mkdir -p "$(DMG_MOUNT_DIR)"; \
	hdiutil attach -nobrowse -readwrite -mountpoint "$(PWD)/$(DMG_MOUNT_DIR)" "$(TEMP_DMG_PATH)" >/dev/null; \
	cp -R "$(DMG_STAGING_DIR)/." "$(DMG_MOUNT_DIR)"; \
	ln -s /Applications "$(DMG_MOUNT_DIR)/Applications"; \
	SetFile -a V "$(DMG_MOUNT_DIR)/$(DMG_BACKGROUND_DIR)"; \
	osascript "$(DMG_WINDOW_SCRIPT)" "$(PWD)/$(DMG_MOUNT_DIR)" "$(APP_NAME).app" "$(DMG_BACKGROUND_NAME)"; \
	hdiutil detach "$(PWD)/$(DMG_MOUNT_DIR)" >/dev/null; \
	rm -rf "$(DMG_MOUNT_DIR)"; \
	hdiutil convert "$(TEMP_DMG_PATH)" -ov -format UDZO -o "$(NOTARIZED_DMG_PATH)"; \
	rm -f "$(TEMP_DMG_PATH)"; \
	codesign --force --timestamp --sign "$$MACOS_SIGNING_IDENTITY" --keychain "$$KEYCHAIN_PATH" "$(NOTARIZED_DMG_PATH)"

staple-notarized-dmg:
	xcrun stapler staple -v "$(NOTARIZED_DMG_PATH)"

write-release-checksums:
	shasum -a 256 "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" > "$(CHECKSUMS_PATH)"

verify-notarized-dmg:
	@set -euo pipefail; \
	MOUNT_POINT=""; \
	cleanup() { \
		if [ -n "$$MOUNT_POINT" ]; then \
			hdiutil detach "$$MOUNT_POINT" >/dev/null 2>&1 || true; \
		fi; \
	}; \
	trap cleanup EXIT; \
	if [ ! -f "$(NOTARIZED_DMG_PATH)" ]; then \
		echo "error: notarized dmg not found at $(NOTARIZED_DMG_PATH)"; \
		exit 1; \
	fi; \
	spctl -a -vv --type install "$(NOTARIZED_DMG_PATH)"; \
	MOUNT_POINT="$$(hdiutil attach -nobrowse -readonly "$(NOTARIZED_DMG_PATH)" | awk 'END{print $$3}')"; \
	if [ ! -L "$$MOUNT_POINT/Applications" ]; then \
		echo "error: dmg is missing Applications symlink"; \
		exit 1; \
	fi; \
	if [ ! -f "$$MOUNT_POINT/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)" ]; then \
		echo "error: dmg is missing background image"; \
		exit 1; \
	fi; \
	spctl -a -vv --type execute "$$MOUNT_POINT/$(APP_NAME).app"; \
	hdiutil detach "$$MOUNT_POINT"; \
	MOUNT_POINT=""

release-notarized: release-pre-notarize
	@set -euo pipefail; \
	xcrun notarytool submit "$(NOTARIZED_DMG_PATH)" --apple-id "$$MACOS_NOTARY_APPLE_ID" --password "$$MACOS_NOTARY_APP_PASSWORD" --team-id "$$MACOS_NOTARY_TEAM_ID" --wait; \
	$(MAKE) staple-notarized-dmg TAG="$(TAG)"; \
	$(MAKE) verify-notarized-dmg TAG="$(TAG)"; \
	$(MAKE) write-release-checksums TAG="$(TAG)"

clean:
	rm -rf "$(APP_BUNDLE)"
	rm -rf "$(DMG_STAGING_DIR)"
	rm -rf "$(DMG_MOUNT_DIR)"
	rm -rf "$(APP_ICONSET_DIR)"
	rm -f "$(APP_ICON_SOURCE)"
	rm -f "$(TEMP_DMG_PATH)"
	rm -f "$(DMG_PATH)" "$(ZIP_PATH)" "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" "$(CHECKSUMS_PATH)"
	rm -f "$(DIST_DIR)/developer-id.p12"
	rm -f "$(DIST_DIR)/downshift-signing.keychain-db" "$(DIST_DIR)/downshift-signing.keychain-db-db"
