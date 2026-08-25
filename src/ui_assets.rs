use std::sync::OnceLock;

const INLINE_STYLE_PLACEHOLDER: &str = "__DOWNSHIFT_INLINE_STYLE__";
const INLINE_SCRIPT_PLACEHOLDER: &str = "__DOWNSHIFT_INLINE_SCRIPT__";

const BREATH_HTML_TEMPLATE: &str = include_str!("ui/breath.html");
const BREATH_CSS: &str = include_str!("ui/breath.css");
// Keep the embedded app and standalone Pages demo on the same polygon math.
const BREATH_POLYGON_JS: &str = include_str!("ui/polygon-animation.js");
const BREATH_JS: &str = include_str!("ui/breath.js");

const TELEMETRY_INFO_HTML_TEMPLATE: &str = include_str!("ui/telemetry-info.html");
const TELEMETRY_INFO_CSS: &str = include_str!("ui/telemetry-info.css");
const TELEMETRY_INFO_JS: &str = include_str!("ui/telemetry-info.js");

const UPDATE_DIALOG_HTML_TEMPLATE: &str = include_str!("ui/update-dialog.html");
const UPDATE_DIALOG_CSS: &str = include_str!("ui/update-dialog.css");
const UPDATE_DIALOG_JS: &str = include_str!("ui/update-dialog.js");

const CUSTOM_SNOOZE_HTML_TEMPLATE: &str = include_str!("ui/custom-snooze.html");
const CUSTOM_SNOOZE_CSS: &str = include_str!("ui/custom-snooze.css");
const CUSTOM_SNOOZE_JS: &str = include_str!("ui/custom-snooze.js");

const BREATHING_PATTERN_HTML_TEMPLATE: &str = include_str!("ui/breathing-pattern.html");
const BREATHING_PATTERN_CSS: &str = include_str!("ui/breathing-pattern.css");
const BREATHING_PATTERN_JS: &str = include_str!("ui/breathing-pattern.js");

static BREATH_HTML: OnceLock<String> = OnceLock::new();
static BREATH_JS_COMBINED: OnceLock<String> = OnceLock::new();
static TELEMETRY_INFO_HTML: OnceLock<String> = OnceLock::new();
static UPDATE_DIALOG_HTML: OnceLock<String> = OnceLock::new();
static CUSTOM_SNOOZE_HTML: OnceLock<String> = OnceLock::new();
static BREATHING_PATTERN_HTML: OnceLock<String> = OnceLock::new();

pub(crate) fn inline_ui_assets(template: &str, css: &str, js: &str) -> String {
    template
        .replace(INLINE_STYLE_PLACEHOLDER, css.trim())
        .replace(INLINE_SCRIPT_PLACEHOLDER, js.trim())
}

fn breath_js() -> &'static str {
    BREATH_JS_COMBINED.get_or_init(|| format!("{BREATH_POLYGON_JS}\n{BREATH_JS}"))
}

pub(crate) fn breath_html() -> &'static str {
    BREATH_HTML.get_or_init(|| inline_ui_assets(BREATH_HTML_TEMPLATE, BREATH_CSS, breath_js()))
}

pub(crate) fn telemetry_info_html() -> &'static str {
    TELEMETRY_INFO_HTML.get_or_init(|| {
        inline_ui_assets(
            TELEMETRY_INFO_HTML_TEMPLATE,
            TELEMETRY_INFO_CSS,
            TELEMETRY_INFO_JS,
        )
    })
}

pub(crate) fn update_dialog_html() -> &'static str {
    UPDATE_DIALOG_HTML.get_or_init(|| {
        inline_ui_assets(
            UPDATE_DIALOG_HTML_TEMPLATE,
            UPDATE_DIALOG_CSS,
            UPDATE_DIALOG_JS,
        )
    })
}

pub(crate) fn custom_snooze_html() -> &'static str {
    CUSTOM_SNOOZE_HTML.get_or_init(|| {
        inline_ui_assets(
            CUSTOM_SNOOZE_HTML_TEMPLATE,
            CUSTOM_SNOOZE_CSS,
            CUSTOM_SNOOZE_JS,
        )
    })
}

pub(crate) fn breathing_pattern_html() -> &'static str {
    BREATHING_PATTERN_HTML.get_or_init(|| {
        inline_ui_assets(
            BREATHING_PATTERN_HTML_TEMPLATE,
            BREATHING_PATTERN_CSS,
            BREATHING_PATTERN_JS,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_ui_assets_replaces_style_and_script_placeholders() {
        let html = inline_ui_assets(
            "<style>__DOWNSHIFT_INLINE_STYLE__</style><script>__DOWNSHIFT_INLINE_SCRIPT__</script>",
            "\nbody { color: red; }\n",
            "\nconsole.log('ok');\n",
        );

        assert_eq!(
            html,
            "<style>body { color: red; }</style><script>console.log('ok');</script>"
        );
    }

    #[test]
    fn breath_html_embeds_polygon_animation_before_breathing_consumer() {
        let html = breath_html();
        let polygon_module = html
            .find("window.downshiftPolygonAnimation")
            .expect("shared polygon module should be embedded");
        let breathing_consumer = html
            .find("terminalHitTargetSizePx")
            .expect("breathing consumer should be embedded");

        assert!(polygon_module < breathing_consumer);
    }
}
