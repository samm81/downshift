# https://tech.davis-hansson.com/p/make/
SHELL := bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.DEFAULT_GOAL := help

# ANSI color codes
COLOR ?= 0
ifeq ($(COLOR),1)
GREEN := $(shell tput -Txterm setaf 2 2>/dev/null)
YELLOW := $(shell tput -Txterm setaf 3 2>/dev/null)
RED := $(shell tput -Txterm setaf 1 2>/dev/null)
BLUE := $(shell tput -Txterm setaf 6 2>/dev/null)
RESET := $(shell tput -Txterm sgr0 2>/dev/null)
else
GREEN :=
YELLOW :=
RED :=
BLUE :=
RESET :=
endif

# !! `Makefile` must use tabs !!

ROOT_DIR := $(CURDIR)
APP_NAME := Downshift
BIN_NAME := downshift
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
BUNDLE_ID := app.getdownshift
MIN_MACOS := 13.0
MACOS_TARGET ?= $(shell rustc -vV 2>/dev/null | sed -n 's/^host: //p')
WINDOWS_VM_ROOT ?= ../downshift-vm
DIST_DIR ?= dist
PAGES_SOURCE_DIR ?= docs
PAGES_DIR ?= $(DIST_DIR)/pages
PAGES_MANIFEST := $(PAGES_SOURCE_DIR)/release.json
PAGES_INDEX := $(PAGES_SOURCE_DIR)/index.html
PAGES_SOURCE_FILES := $(shell find "$(PAGES_SOURCE_DIR)" -type f -print)
PAGES_SOURCE_DIRS := $(shell find "$(PAGES_SOURCE_DIR)" -type d -print)
PAGES_BUILD_INPUTS := $(sort $(PAGES_MANIFEST) $(PAGES_SOURCE_FILES) $(PAGES_SOURCE_DIRS) dev/pages/build-site.mjs dev/pages/release-manifest.mjs)
PAGES_STAMP := $(PAGES_DIR)/.build.stamp
PAGES_PREVIEW_HOST ?= 127.0.0.1
PAGES_PREVIEW_BIND ?= 0.0.0.0
PAGES_PREVIEW_PORT ?= 4173
TAG ?=
RESET ?=
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

TELEMETRY_GOALS := $(filter build build-macos build-debug package-app sign-app stage-dmg-contents app-macos zip dmg checksums release release-pre-notarize release-notarized,$(MAKECMDGOALS))
NO_TELEMETRY_GOALS := $(filter build-no-telemetry build-macos-no-telemetry build-debug-no-telemetry package-app-no-telemetry sign-app-no-telemetry stage-dmg-contents-no-telemetry app-macos-no-telemetry zip-no-telemetry dmg-no-telemetry checksums-no-telemetry release-no-telemetry,$(MAKECMDGOALS))
ifneq ($(strip $(TAG)),)
ifneq ($(TAG),v$(VERSION))
$(error TAG must match the Cargo version exactly, for example TAG=v$(VERSION))
endif
endif
ifneq ($(strip $(TELEMETRY_GOALS)),)
ifneq ($(strip $(NO_TELEMETRY_GOALS)),)
$(error choose either telemetry-enabled or no-telemetry artifact goals per make invocation)
endif
endif

.PHONY: help \
	build build-no-telemetry \
	build-macos build-macos-no-telemetry \
	verify-windows build-windows-installer smoke-windows \
	build-debug build-debug-no-telemetry \
	run run-no-telemetry \
	smoke-windows-vm \
	pages-build pages-preview pages-check pages-smoke \
	app-macos app-macos-no-telemetry \
	format format-check test lint check verify-rust verify-release \
	sign-app sign-app-no-telemetry \
	generate-app-icon generate-dmg-background \
	stage-dmg-contents stage-dmg-contents-no-telemetry \
	dmg dmg-no-telemetry \
	zip zip-no-telemetry \
	release-pre-notarize \
	staple-notarized-dmg \
	write-release-checksums \
	checksums checksums-no-telemetry \
	verify-notarized-dmg \
	check-tag-sync require-macos-target require-telemetry-env \
	require-notarization-env require-safe-output-paths \
	package-app package-app-no-telemetry \
	release release-no-telemetry release-notarized clean

help: ## show available targets
	@printf '%s\n' "$(GREEN)targets:$(RESET)"
	grep -E '^[a-zA-Z0-9_./-]+:.*## .+$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  %-28s %s\n", $$1, $$2}'

format: ## format Rust, web, and shell sources
	cargo fmt
	npm run fmt:web
	npm run fmt:shell

format-check: ## check Rust, web, and shell formatting
	cargo fmt --check
	npm run fmt:check:web
	npm run fmt:check:shell

test: ## run Rust tests
	cargo test --locked

lint: ## run Rust and web lint checks
	cargo clippy --locked --all-targets -- -D warnings
	npm run lint

check: verify-rust ## run the complete local verification gate
	npm run check

verify-rust: ## run Rust formatting, tests, and Clippy
	cargo fmt --check
	cargo test --locked
	cargo clippy --locked --all-targets -- -D warnings

verify-release: check ## verify Rust, web, shell, Markdown, and Pages checks

require-telemetry-env:
	@for name in \
		DOWNSHIFT_TELEMETRY_ENABLED \
		DOWNSHIFT_BETTERSTACK_LOGS_TOKEN \
		DOWNSHIFT_BETTERSTACK_LOGS_HOST \
		DOWNSHIFT_BETTERSTACK_ERRORS_DSN \
		DOWNSHIFT_BUILD_CHANNEL; do
		if [ -z "$${!name:-}" ]; then
			echo "error: $$name is required"
			exit 1
		fi
	done
	if [ "$${DOWNSHIFT_TELEMETRY_ENABLED}" != "true" ]; then
		echo "error: DOWNSHIFT_TELEMETRY_ENABLED must be 'true' for default telemetry-enabled targets"
		exit 1
	fi

require-notarization-env:
	@for name in \
		MACOS_CERT_P12_B64 \
		MACOS_CERT_P12_PASSWORD \
		MACOS_KEYCHAIN_PASSWORD \
		MACOS_SIGNING_IDENTITY \
		MACOS_NOTARY_APPLE_ID \
		MACOS_NOTARY_APP_PASSWORD \
		MACOS_NOTARY_TEAM_ID; do
		if [ -z "$${!name:-}" ]; then
			echo "error: $$name is required"
			exit 1
		fi
	done

require-safe-output-paths:
	@root_dir="$$(cd "$(ROOT_DIR)" && pwd -P)"
	validate_output_dir() {
		output_dir="$$1"
		case "$$output_dir" in
			""|.|..|/*)
				echo "error: output directory must be a non-empty relative path: $$output_dir" >&2
				return 1
				;;
		esac
		case "/$$output_dir/" in
			*/./*|*/../*)
				echo "error: output directory must not contain . or .. path segments: $$output_dir" >&2
				return 1
				;;
		esac

		if [ -d "$$output_dir" ]; then
			output_real="$$(cd "$$output_dir" && pwd -P)"
		elif [ -e "$$output_dir" ] || [ -L "$$output_dir" ]; then
			echo "error: output path is not a directory: $$output_dir" >&2
			return 1
		else
			output_parent="$$(dirname "./$$output_dir")"
			output_base="$$(basename "./$$output_dir")"
			while [ ! -d "$$output_parent" ]; do
				output_base="$$(basename "./$$output_parent")/$$output_base"
				output_parent="$$(dirname "./$$output_parent")"
			done
			output_real="$$(cd "$$output_parent" && pwd -P)/$$output_base"
		fi
		case "$$output_real" in
			"$$root_dir")
				echo "error: refusing to use the repository root as an output directory" >&2
				return 1
				;;
			"$$root_dir"/*) ;;
			*)
				echo "error: output directory must resolve below the repository: $$output_dir" >&2
				return 1
				;;
		esac
	}

	validate_output_dir "$(DIST_DIR)"
	validate_output_dir "$(PAGES_DIR)"
	case "$(APP_NAME)" in
		""|.|..|*/*|*\\*|*[!A-Za-z0-9._-]*)
			echo "error: APP_NAME must contain only letters, numbers, dots, underscores, or hyphens" >&2
			exit 1
		;;
	esac

build: require-telemetry-env ## build the host-native release binary with telemetry
	cargo build --locked --release

build-no-telemetry: ## build the host-native release binary without telemetry
	cargo build --locked --release

require-macos-target:
	@case "$(MACOS_TARGET)" in
		*-apple-darwin) ;;
		*)
			echo "error: MACOS_TARGET must be an Apple Darwin target (for example aarch64-apple-darwin)" >&2
			exit 1
		;;
	esac
	case "$(MACOS_TARGET)" in
		*[!A-Za-z0-9_-]*)
			echo "error: MACOS_TARGET contains unsupported path characters" >&2
			exit 1
		;;
	esac

build-macos: require-macos-target require-telemetry-env ## build the macOS release binary with telemetry
	cargo build --locked --release --target "$(MACOS_TARGET)"

build-macos-no-telemetry: require-macos-target ## build the macOS release binary without telemetry
	cargo build --locked --release --target "$(MACOS_TARGET)"

verify-windows: ## run Windows host checks and release build
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/fast-check.ps1 -Release

build-windows-installer: ## build the unsigned Windows installer
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/build-installer.ps1

smoke-windows: build-windows-installer ## run the Windows installer smoke test
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/smoke-installer.ps1 -InstallerPath "$(DIST_DIR)/windows/$(APP_NAME)-Setup-$(VERSION).exe"

build-debug: require-telemetry-env ## build the host-native debug binary with telemetry
	cargo build --locked --quiet

build-debug-no-telemetry: ## build the host-native debug binary without telemetry
	cargo build --locked --quiet

smoke-windows-vm: ## run the Windows VM smoke test
	powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File windows/smoke-vm.ps1 -VmRoot "$(WINDOWS_VM_ROOT)"

pages-build: $(PAGES_STAMP) ## build the local GitHub Pages artifact
	test -f "$(PAGES_DIR)/index.html"
	test -f "$(PAGES_DIR)/release.json"

$(PAGES_STAMP): $(PAGES_BUILD_INPUTS) | require-safe-output-paths
	node dev/pages/build-site.mjs build "$(PAGES_MANIFEST)" "$(PAGES_SOURCE_DIR)" "$(PAGES_DIR)"
	test -f "$(PAGES_DIR)/index.html"
	test -f "$(PAGES_DIR)/release.json"
	touch "$@"

pages-preview: pages-build ## build and serve the local GitHub Pages preview
	command -v python3 >/dev/null 2>&1 || { echo "error: python3 is required for the local GitHub Pages preview"; exit 1; }
	echo "GitHub Pages preview: http://$(PAGES_PREVIEW_HOST):$(PAGES_PREVIEW_PORT)/"
	echo "Serving $(ROOT_DIR)/$(PAGES_DIR); press Ctrl-C to stop."
	python3 -m http.server "$(PAGES_PREVIEW_PORT)" --bind "$(PAGES_PREVIEW_BIND)" --directory "$(PAGES_DIR)"

pages-check: pages-build ## validate the local GitHub Pages artifact
	node dev/pages/release-manifest.mjs validate "$(PAGES_MANIFEST)"
	node dev/pages/release-manifest.mjs validate-embedded "$(PAGES_MANIFEST)" "$(PAGES_INDEX)"
	node dev/pages/release-manifest.mjs validate "$(PAGES_DIR)/release.json"
	node dev/pages/release-manifest.mjs validate-embedded "$(PAGES_DIR)/release.json" "$(PAGES_DIR)/index.html"

pages-smoke: pages-check ## run the GitHub Pages browser smoke test
	npm run smoke:pages -- "$(PAGES_DIR)"

run: build-debug ## run the telemetry-enabled debug app
	if [ -n "$(RUN_RESET)" ]; then
		echo "resetting saved downshift settings"
		rm -f -- "$$HOME/Library/Application Support/downshift/settings.toml" "$$HOME/.config/downshift/settings.toml"
	fi
	./target/debug/$(BIN_NAME)

run-no-telemetry: build-debug-no-telemetry ## run the no-telemetry debug app
	if [ -n "$(RUN_RESET)" ]; then
		echo "resetting saved downshift settings"
		rm -f -- "$$HOME/Library/Application Support/downshift/settings.toml" "$$HOME/.config/downshift/settings.toml"
	fi
	./target/debug/$(BIN_NAME)

$(DIST_DIR): | require-safe-output-paths
	mkdir -p "$@"

$(APP_ICON_SOURCE): $(APP_ICON_SCRIPT) $(APP_ICON_BASE_SOURCE) | require-safe-output-paths $(DIST_DIR)
	rm -rf -- "$(APP_ICONSET_DIR)"
	swift "$(APP_ICON_SCRIPT)" "$(APP_ICON_BASE_SOURCE)" "$(APP_ICONSET_DIR)"
	rm -f -- "$(APP_ICON_SOURCE)"
	iconutil -c icns "$(APP_ICONSET_DIR)" -o "$(APP_ICON_SOURCE)"
	test -s "$@"

generate-app-icon: $(APP_ICON_SOURCE) ## generate the macOS app icon

$(DMG_BACKGROUND_SOURCE): $(DMG_BACKGROUND_SCRIPT) | require-safe-output-paths $(DIST_DIR)
	swift "$(DMG_BACKGROUND_SCRIPT)" "$(DMG_BACKGROUND_SOURCE)"
	test -s "$@"

generate-dmg-background: $(DMG_BACKGROUND_SOURCE) ## generate the DMG background image

package-app: build-macos $(APP_ICON_SOURCE) | require-safe-output-paths ## package the telemetry-enabled macOS app
package-app-no-telemetry: build-macos-no-telemetry $(APP_ICON_SOURCE) | require-safe-output-paths ## package the no-telemetry macOS app
package-app package-app-no-telemetry:
	test -x "$(MACOS_RELEASE_BINARY)"
	test -s "$(APP_ICON_SOURCE)"
	rm -rf -- "$(APP_BUNDLE)"
	mkdir -p "$(APP_BUNDLE)/Contents/MacOS" "$(APP_BUNDLE)/Contents/Resources"
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
	test -x "$(APP_BUNDLE)/Contents/MacOS/$(BIN_NAME)"
	test -s "$(APP_BUNDLE)/Contents/Info.plist"

sign-app: package-app
sign-app-no-telemetry: package-app-no-telemetry
sign-app sign-app-no-telemetry:
	codesign --force --deep --sign - "$(APP_BUNDLE)"
	codesign --verify --deep --strict --verbose=2 "$(APP_BUNDLE)"

stage-dmg-contents: app-macos $(DMG_BACKGROUND_SOURCE) | require-safe-output-paths ## stage the telemetry-enabled app for a DMG
stage-dmg-contents-no-telemetry: app-macos-no-telemetry $(DMG_BACKGROUND_SOURCE) | require-safe-output-paths ## stage the no-telemetry app for a DMG
stage-dmg-contents stage-dmg-contents-no-telemetry:
	test -d "$(APP_BUNDLE)"
	test -s "$(DMG_BACKGROUND_SOURCE)"
	rm -rf -- "$(DMG_STAGING_DIR)"
	mkdir -p "$(DMG_STAGING_DIR)"
	cp -R "$(APP_BUNDLE)" "$(DMG_STAGING_DIR)/$(APP_NAME).app"
	mkdir -p "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)"
	cp "$(DMG_BACKGROUND_SOURCE)" "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)"
	test -d "$(DMG_STAGING_DIR)/$(APP_NAME).app"

app-macos: sign-app ## build and sign the telemetry-enabled macOS app

app-macos-no-telemetry: sign-app-no-telemetry ## build and sign the no-telemetry macOS app

zip: app-macos | require-safe-output-paths ## create the telemetry-enabled macOS zip
zip-no-telemetry: app-macos-no-telemetry | require-safe-output-paths ## create the no-telemetry macOS zip
zip zip-no-telemetry:
	test -d "$(APP_BUNDLE)"
	rm -f -- "$(ZIP_PATH)"
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(ZIP_PATH)"
	test -s "$(ZIP_PATH)"

dmg: stage-dmg-contents | require-safe-output-paths ## create the telemetry-enabled macOS DMG
dmg-no-telemetry: stage-dmg-contents-no-telemetry | require-safe-output-paths ## create the no-telemetry macOS DMG
dmg dmg-no-telemetry:
	mount_point="$(ROOT_DIR)/$(DMG_MOUNT_DIR)"
	mounted_mount=""
	completed=0
	cleanup() {
		if [ -n "$$mounted_mount" ]; then
			if hdiutil detach "$$mounted_mount" >/dev/null 2>&1; then
				mounted_mount=""
			fi
		fi
		if [ -z "$$mounted_mount" ]; then
			rm -rf -- "$(DMG_MOUNT_DIR)"
		fi
		rm -f -- "$(TEMP_DMG_PATH)"
		if [ "$$completed" -ne 1 ]; then
			rm -f -- "$(DMG_PATH)"
		fi
	}
	trap cleanup EXIT
	hdiutil detach "$$mount_point" >/dev/null 2>&1 || true
	rm -rf -- "$(DMG_MOUNT_DIR)"
	rm -f -- "$(TEMP_DMG_PATH)" "$(DMG_PATH)"
	size_mb="$$(du -sm "$(DMG_STAGING_DIR)" | awk '{print $$1 + 32}')"
	hdiutil create -size "$${size_mb}m" -fs HFS+ -volname "$(APP_NAME)" -ov "$(TEMP_DMG_PATH)"
	mkdir -p "$(DMG_MOUNT_DIR)"
	hdiutil attach -nobrowse -readwrite -mountpoint "$$mount_point" "$(TEMP_DMG_PATH)" >/dev/null
	mounted_mount="$$mount_point"
	cp -R "$(DMG_STAGING_DIR)/." "$(DMG_MOUNT_DIR)"
	ln -s /Applications "$(DMG_MOUNT_DIR)/Applications"
	SetFile -a V "$(DMG_MOUNT_DIR)/$(DMG_BACKGROUND_DIR)"
	osascript "$(DMG_WINDOW_SCRIPT)" "$$mount_point" "$(APP_NAME).app" "$(DMG_BACKGROUND_NAME)"
	hdiutil detach "$$mount_point" >/dev/null
	mounted_mount=""
	rm -rf -- "$(DMG_MOUNT_DIR)"
	hdiutil convert "$(TEMP_DMG_PATH)" -ov -format UDZO -o "$(DMG_PATH)"
	test -s "$(DMG_PATH)"
	completed=1
	trap - EXIT
	rm -f -- "$(TEMP_DMG_PATH)"

checksums: zip dmg | require-safe-output-paths ## write checksums for the unsigned release archives
checksums-no-telemetry: zip-no-telemetry dmg-no-telemetry | require-safe-output-paths ## write checksums for no-telemetry release archives
checksums checksums-no-telemetry:
	test -s "$(ZIP_PATH)"
	test -s "$(DMG_PATH)"
	checksum_tmp="$$(mktemp "$(CHECKSUMS_PATH).XXXXXX")"
	cleanup() { rm -f -- "$$checksum_tmp"; }
	trap cleanup EXIT
	shasum -a 256 "$(ZIP_PATH)" "$(DMG_PATH)" > "$$checksum_tmp"
	mv "$$checksum_tmp" "$(CHECKSUMS_PATH)"
	trap - EXIT

check-tag-sync:
	@if [ -n "$(TAG)" ] && [ "$(TAG)" != "v$(VERSION)" ]; then
		echo "error: tag $(TAG) does not match cargo version $(VERSION)"
		exit 1
	fi

release: check-tag-sync checksums ## create unsigned macOS release archives

release-no-telemetry: check-tag-sync checksums-no-telemetry ## create unsigned no-telemetry macOS release archives

# Newer macOS runner images ship OpenSSL 3, which needs the legacy provider for older .p12 bundles.
release-pre-notarize: check-tag-sync require-telemetry-env require-notarization-env require-safe-output-paths build-macos package-app generate-dmg-background ## create signed macOS release assets before notarization
	set -euo pipefail; \
	KEYCHAIN_PATH="$(ROOT_DIR)/$(DIST_DIR)/downshift-signing.keychain-db"; \
	CERT_PATH="$(ROOT_DIR)/$(DIST_DIR)/developer-id.p12"; \
	PKCS12_ERROR_PATH="$(DIST_DIR)/pkcs12-validation.error"; \
	completed=0; \
	cleanup() { \
		security delete-keychain "$$KEYCHAIN_PATH" >/dev/null 2>&1 || true; \
		rm -f -- "$$CERT_PATH" "$$PKCS12_ERROR_PATH"; \
		if [ "$$completed" -ne 1 ]; then \
			hdiutil detach "$(ROOT_DIR)/$(DMG_MOUNT_DIR)" >/dev/null 2>&1 || true; \
			rm -rf -- "$(DMG_MOUNT_DIR)"; \
			rm -f -- "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" "$(CHECKSUMS_PATH)" "$(TEMP_DMG_PATH)"; \
		fi; \
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
	rm -f -- "$$PKCS12_ERROR_PATH"; \
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
	rm -f -- "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" "$(CHECKSUMS_PATH)"; \
	ditto -c -k --sequesterRsrc --keepParent "$(APP_BUNDLE)" "$(SIGNED_ZIP_PATH)"; \
	rm -rf -- "$(DMG_STAGING_DIR)"; \
	mkdir -p "$(DMG_STAGING_DIR)"; \
	cp -R "$(APP_BUNDLE)" "$(DMG_STAGING_DIR)/$(APP_NAME).app"; \
	mkdir -p "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)"; \
	cp "$(DMG_BACKGROUND_SOURCE)" "$(DMG_STAGING_DIR)/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)"; \
	rm -f -- "$(TEMP_DMG_PATH)"; \
	hdiutil detach "$(ROOT_DIR)/$(DMG_MOUNT_DIR)" >/dev/null 2>&1 || true; \
	rm -rf -- "$(DMG_MOUNT_DIR)"; \
	SIZE_MB="$$(du -sm "$(DMG_STAGING_DIR)" | awk '{print $$1 + 32}')"; \
	hdiutil create -size "$${SIZE_MB}m" -fs HFS+ -volname "$(APP_NAME)" -ov "$(TEMP_DMG_PATH)"; \
	mkdir -p "$(DMG_MOUNT_DIR)"; \
	hdiutil attach -nobrowse -readwrite -mountpoint "$(ROOT_DIR)/$(DMG_MOUNT_DIR)" "$(TEMP_DMG_PATH)" >/dev/null; \
	cp -R "$(DMG_STAGING_DIR)/." "$(DMG_MOUNT_DIR)"; \
	ln -s /Applications "$(DMG_MOUNT_DIR)/Applications"; \
	SetFile -a V "$(DMG_MOUNT_DIR)/$(DMG_BACKGROUND_DIR)"; \
	osascript "$(DMG_WINDOW_SCRIPT)" "$(ROOT_DIR)/$(DMG_MOUNT_DIR)" "$(APP_NAME).app" "$(DMG_BACKGROUND_NAME)"; \
	hdiutil detach "$(ROOT_DIR)/$(DMG_MOUNT_DIR)" >/dev/null; \
	rm -rf -- "$(DMG_MOUNT_DIR)"; \
	hdiutil convert "$(TEMP_DMG_PATH)" -ov -format UDZO -o "$(NOTARIZED_DMG_PATH)"; \
	rm -f -- "$(TEMP_DMG_PATH)"; \
	codesign --force --timestamp --sign "$$MACOS_SIGNING_IDENTITY" --keychain "$$KEYCHAIN_PATH" "$(NOTARIZED_DMG_PATH)"; \
	test -s "$(SIGNED_ZIP_PATH)"; \
	test -s "$(NOTARIZED_DMG_PATH)"; \
	completed=1

staple-notarized-dmg: require-safe-output-paths ## staple the notarized macOS DMG
	test -s "$(NOTARIZED_DMG_PATH)"
	xcrun stapler staple -v "$(NOTARIZED_DMG_PATH)"

write-release-checksums: require-safe-output-paths ## write checksums for signed release assets
	test -s "$(SIGNED_ZIP_PATH)"
	test -s "$(NOTARIZED_DMG_PATH)"
	checksum_tmp="$$(mktemp "$(CHECKSUMS_PATH).XXXXXX")"
	cleanup() { rm -f -- "$$checksum_tmp"; }
	trap cleanup EXIT
	shasum -a 256 "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" > "$$checksum_tmp"
	mv "$$checksum_tmp" "$(CHECKSUMS_PATH)"
	trap - EXIT

verify-notarized-dmg: require-safe-output-paths ## validate the notarized macOS DMG
	MOUNT_POINT=""
	cleanup() {
		if [ -n "$$MOUNT_POINT" ]; then
			hdiutil detach "$$MOUNT_POINT" >/dev/null 2>&1 || true
		fi
	}
	trap cleanup EXIT
	if [ ! -s "$(NOTARIZED_DMG_PATH)" ]; then
		echo "error: notarized dmg not found at $(NOTARIZED_DMG_PATH)"
		exit 1
	fi
	spctl -a -vv --type install "$(NOTARIZED_DMG_PATH)"
	MOUNT_POINT="$$(hdiutil attach -nobrowse -readonly "$(NOTARIZED_DMG_PATH)" | awk 'END{print $$3}')"
	if [ ! -L "$$MOUNT_POINT/Applications" ]; then
		echo "error: dmg is missing Applications symlink"
		exit 1
	fi
	if [ ! -f "$$MOUNT_POINT/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)" ]; then
		echo "error: dmg is missing background image"
		exit 1
	fi
	spctl -a -vv --type execute "$$MOUNT_POINT/$(APP_NAME).app"
	hdiutil detach "$$MOUNT_POINT"
	MOUNT_POINT=""

release-notarized: release-pre-notarize ## submit, staple, and validate the notarized macOS DMG
	xcrun notarytool submit "$(NOTARIZED_DMG_PATH)" --apple-id "$$MACOS_NOTARY_APPLE_ID" --password "$$MACOS_NOTARY_APP_PASSWORD" --team-id "$$MACOS_NOTARY_TEAM_ID" --wait
	xcrun stapler staple -v "$(NOTARIZED_DMG_PATH)"
	MOUNT_POINT=""
	cleanup_verify() {
		if [ -n "$$MOUNT_POINT" ]; then
			hdiutil detach "$$MOUNT_POINT" >/dev/null 2>&1 || true
		fi
	}
	trap cleanup_verify EXIT
	if [ ! -s "$(NOTARIZED_DMG_PATH)" ]; then
		echo "error: notarized dmg not found at $(NOTARIZED_DMG_PATH)"
		exit 1
	fi
	spctl -a -vv --type install "$(NOTARIZED_DMG_PATH)"
	MOUNT_POINT="$$(hdiutil attach -nobrowse -readonly "$(NOTARIZED_DMG_PATH)" | awk 'END{print $$3}')"
	if [ ! -L "$$MOUNT_POINT/Applications" ]; then
		echo "error: dmg is missing Applications symlink"
		exit 1
	fi
	if [ ! -f "$$MOUNT_POINT/$(DMG_BACKGROUND_DIR)/$(DMG_BACKGROUND_NAME)" ]; then
		echo "error: dmg is missing background image"
		exit 1
	fi
	spctl -a -vv --type execute "$$MOUNT_POINT/$(APP_NAME).app"
	hdiutil detach "$$MOUNT_POINT"
	MOUNT_POINT=""
	trap - EXIT
	test -s "$(SIGNED_ZIP_PATH)"
	checksum_tmp="$$(mktemp "$(CHECKSUMS_PATH).XXXXXX")"
	cleanup_checksums() { rm -f -- "$$checksum_tmp"; }
	trap cleanup_checksums EXIT
	shasum -a 256 "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" > "$$checksum_tmp"
	mv "$$checksum_tmp" "$(CHECKSUMS_PATH)"
	trap - EXIT

clean: require-safe-output-paths ## remove generated build and release artifacts
	if command -v hdiutil >/dev/null 2>&1 && [ -d "$(DMG_MOUNT_DIR)" ] && [ ! -L "$(DMG_MOUNT_DIR)" ]; then
		hdiutil detach "$(ROOT_DIR)/$(DMG_MOUNT_DIR)" >/dev/null 2>&1 || true
	fi
	rm -rf -- "$(APP_BUNDLE)"
	rm -rf -- "$(DMG_STAGING_DIR)"
	rm -rf -- "$(DMG_MOUNT_DIR)"
	rm -rf -- "$(APP_ICONSET_DIR)"
	rm -rf -- "$(PAGES_DIR)"
	rm -f -- "$(APP_ICON_SOURCE)"
	rm -f -- "$(DMG_BACKGROUND_SOURCE)"
	rm -f -- "$(TEMP_DMG_PATH)"
	rm -f -- "$(DMG_PATH)" "$(ZIP_PATH)" "$(SIGNED_ZIP_PATH)" "$(NOTARIZED_DMG_PATH)" "$(CHECKSUMS_PATH)"
	rm -f -- "$(DIST_DIR)/developer-id.p12" "$(DIST_DIR)/pkcs12-validation.error"
	rm -f -- "$(DIST_DIR)/downshift-signing.keychain-db" "$(DIST_DIR)/downshift-signing.keychain-db-db"
